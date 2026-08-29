//! The machine-global store for compiled rule-host binaries: one place on a
//! machine for a build already done.
//!
//! A repository keeps its own cache under `.polint/cache`, and that cache is per
//! checkout. Every git worktree of one repository therefore recompiles the same
//! rule host from the same bytes, and a machine with a dozen worktrees pays the
//! same four-to-seven minute cold build a dozen times. The store is where that
//! build is shared.
//!
//! ## What makes sharing legitimate
//!
//! Nothing here decides a fact. Every entry is written under a key that already
//! names the complete input surface of the build that produced it — the
//! compiler and its flags, every manifest and lockfile cargo reads, and the
//! content of every rule source — and every entry records that key INSIDE
//! itself, so an entry can only be offered as the answer to the exact question
//! it answered. Same key, same inputs, same binary.
//!
//! That obligation is the caller's, and
//! [`inputs_are_the_same_from_every_checkout`] is where it is discharged: a rule
//! package whose build reads sources the fingerprint cannot name never reaches
//! the store at all. It is compiled locally instead, which is slower and never
//! wrong.
//!
//! An absolute `path` dependency names one directory on this machine, so a
//! fingerprint computed in any checkout identifies the same bytes; a relative
//! one that leaves the rule package names a different directory in each
//! checkout, and two checkouts with byte-identical rule sources would then
//! compute one key over two different builds. That is the same rule the CI
//! action's build-deps digest already exposes, and it is why absolute paths are
//! shareable and escaping relative paths are not.
//!
//! ## Trust
//!
//! The store is **one user's state on one machine**, at the trust level of
//! `~/.cargo/registry`. polint creates it private to the invoking user (`0700`
//! on Unix) and never shares it between users, over a network, or with a remote.
//! Every restore re-hashes the bytes it copied before anything runs them.
//!
//! Content verification catches corruption — a truncated copy, a half-written
//! file, a bad disk. It cannot make a directory that OTHER users can write into
//! safe, because whoever can rewrite an entry can rewrite the hash beside it.
//! Point [`POLINT_CACHE_STORE_ENV`] at local storage only you can write, exactly
//! as you would a cargo registry — not at a share several machines mount. A rule
//! host is a native executable, and while the fingerprint covers the compiler
//! and its flags, it says nothing about the system libraries the machine that
//! built it linked against.
//!
//! ## Failure is a miss, never an error
//!
//! No read, write, or path resolution here can fail a run. A store that does not
//! exist, cannot be created, is full, or holds a corrupt entry is a *miss*: the
//! caller compiles the host the one way it always could. Deleting the whole
//! directory is always safe.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Overrides where the store lives, or turns sharing off.
///
/// An absolute path names the store directory. `off`, `disabled`, or `none` (any
/// casing) turns sharing off, leaving each checkout with only its own
/// `.polint/cache`. Unset means the platform's user cache directory. Any other
/// value — a relative path, an empty string — is not a location polint can
/// resolve, so sharing is off for that run.
pub(crate) const POLINT_CACHE_STORE_ENV: &str = "POLINT_CACHE_STORE";

/// The schema tag every store entry and build stamp carries, and the first line
/// of every fingerprint. Bumping it retires both at once.
const SCHEMA: &str = "polint-rule-host-store-v1";

/// The directory polint keeps under the platform cache directory.
const CACHE_DIR_NAME: &str = "polint";

/// The store's directory inside polint's cache directory.
const STORE_DIR_NAME: &str = "store";

/// The values of [`POLINT_CACHE_STORE_ENV`] that turn sharing off.
const OFF_VALUES: [&str; 3] = ["off", "disabled", "none"];

/// The longest key the store will build a path from.
///
/// Every key polint writes is a sha256 hex digest, which is 64. The bound exists
/// so that a key is a file name and can never become anything else.
const MAX_KEY_LEN: usize = 128;

/// The file recording which rule host the pinned cargo target directory holds.
pub(crate) const STAMP_FILE_NAME: &str = "polint-store-stamp.json";

/// The digest recorded for a file that is not there.
const ABSENT: &str = "absent";

/// The value recorded for an environment variable that is not set, matching the
/// CI action's build-env digest.
const UNSET: &str = "<unset>";

/// The environment variables that change what the compiler produces from
/// unchanged sources, and the fingerprint line each one is recorded on.
///
/// Cargo reads all of them from the environment polint spawns it in, and none of
/// them leaves a trace in any file the fingerprint hashes.
const CARGO_FLAG_VARIABLES: [(&str, &str); 7] = [
    ("rustflags", "RUSTFLAGS"),
    ("cargo_encoded_rustflags", "CARGO_ENCODED_RUSTFLAGS"),
    ("cargo_build_rustflags", "CARGO_BUILD_RUSTFLAGS"),
    ("cargo_build_target", "CARGO_BUILD_TARGET"),
    ("cargo_incremental", "CARGO_INCREMENTAL"),
    ("rustc_wrapper", "RUSTC_WRAPPER"),
    ("rustc_workspace_wrapper", "RUSTC_WORKSPACE_WRAPPER"),
];

/// The names cargo reads a config from in a `.cargo/` directory: the current one
/// and the extensionless one it kept honoring for projects written before 1.39.
const CARGO_CONFIG_NAMES: [&str; 2] = ["config.toml", "config"];

/// The cargo config keys that can put a package's source somewhere other than
/// where the manifest and the lockfile say it is.
///
/// `patch` and `replace` send a named package to another source and `paths`
/// overrides whatever packages it finds in the directories it lists; none of
/// that is recorded in a lockfile, so the rule package can be byte-identical
/// either side of such a config while the sources compiled through it are not.
/// `include` is here because this never follows one: a config whose full content
/// cannot be established is never one this claims to have proven.
const CARGO_REDIRECT_KEYS: [&str; 4] = ["patch", "replace", "paths", "include"];

/// A machine-global content-addressed store rooted at one directory.
#[derive(Debug, Clone)]
pub(crate) struct RuleHostStore {
    root: PathBuf,
}

impl RuleHostStore {
    /// The store this environment asks for, or `None` when sharing is off or has
    /// no location.
    ///
    /// This is the one place polint reads [`POLINT_CACHE_STORE_ENV`]. Everything
    /// below it takes the resolved store as an argument, so a library call can
    /// never depend on the ambient environment and a test can never reach the
    /// developer's own store by accident.
    pub(crate) fn from_env() -> Option<Self> {
        let root = resolve_root(
            std::env::var(POLINT_CACHE_STORE_ENV).ok().as_deref(),
            std::env::var("XDG_CACHE_HOME").ok().as_deref(),
            std::env::var("LOCALAPPDATA").ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        )?;
        Some(Self::at(root))
    }

    /// A store rooted at an explicit directory.
    pub(crate) fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The bytes recorded for `key`, or `None` when nothing is recorded.
    fn read(&self, key: &str) -> Option<Vec<u8>> {
        std::fs::read(self.entry_path(key)?).ok()
    }

    /// Record `bytes` for `key`, doing nothing at all if that is not possible.
    fn publish(&self, key: &str, bytes: &[u8]) {
        let Some(path) = self.entry_path(key) else {
            return;
        };
        let _ = write_atomically(&path, bytes);
    }

    /// Delete the entry for `key`, which a caller does when it proved the entry
    /// is not usable.
    fn discard(&self, key: &str) {
        if let Some(path) = self.entry_path(key) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Where the blob whose sha256 hex digest is `hash` lives.
    ///
    /// A blob is named by its own content, so two publishers of different bytes
    /// never write the same file and a published blob is never rewritten in
    /// place.
    fn blob_path(&self, hash: &str) -> Option<PathBuf> {
        Some(self.root.join("blobs").join(shard(hash)?).join(hash))
    }

    /// Copy `source` into the store under its content hash, doing nothing if
    /// that is not possible.
    ///
    /// The caller has already hashed `source` — that hash is what the entry
    /// pointing at this blob records — so the store takes it rather than reading
    /// the file a second time.
    ///
    /// A blob that is already there is left alone: it is named by its own
    /// content, so re-copying it could only produce the same bytes. If it has
    /// since been corrupted, the restore that reads it removes it, and the
    /// publish after the next build writes it again.
    fn publish_blob(&self, hash: &str, source: &Path) {
        let Some(path) = self.blob_path(hash) else {
            return;
        };
        if path.is_file() {
            return;
        }
        let Some(parent) = path.parent() else {
            return;
        };
        if create_dir_all_private(parent).is_err() {
            return;
        }
        let temporary = temporary_beside(&path);
        if std::fs::copy(source, &temporary).is_err() || std::fs::rename(&temporary, &path).is_err()
        {
            let _ = std::fs::remove_file(&temporary);
        }
    }

    /// The file an entry for `key` is stored in, or `None` when `key` is not a
    /// key this store writes.
    fn entry_path(&self, key: &str) -> Option<PathBuf> {
        Some(
            self.root
                .join("rule-hosts")
                .join(shard(key)?)
                .join(format!("{key}.json")),
        )
    }
}

/// What the store records about one compiled rule host: which blob is the
/// binary, where it belongs under the cargo target directory, and what it must
/// hash to.
///
/// The key is recorded INSIDE the entry as well as in its file name, and the
/// polint version beside it: an entry must be able to prove which question it
/// answers, so that a rewritten or restored store directory cannot present one
/// answer as another. A store holds entries from every polint version that ever
/// wrote to it, and a version that does not match this one is a miss.
#[derive(Debug, Serialize, Deserialize)]
struct StoreEntry {
    schema: String,
    key: String,
    polint_version: String,
    /// The binary's path relative to the rule host's cargo target directory,
    /// such as `release/polint-local-rules`.
    target_relative_path: String,
    sha256: String,
    len: u64,
}

/// The recorded identity of the rule host already in a cargo target directory.
///
/// This is what lets a warm run skip cargo altogether: the fingerprint names the
/// build's whole input surface, so a stamp that still matches means the binary
/// beside it is the one cargo would produce. The length and digest are re-read
/// from disk on every check, so a truncated or replaced binary is never run on
/// the strength of a stale record.
///
/// One target directory holds one stamp. A repository with several rule packages
/// pointed at the same target directory therefore keeps the stamp of whichever
/// package built last; the others find a fingerprint that is not theirs, which
/// is a miss like any other and sends them to the store or to cargo.
#[derive(Debug, Serialize, Deserialize)]
struct HostStamp {
    schema: String,
    fingerprint: String,
    target_relative_path: String,
    sha256: String,
    len: u64,
}

/// Whether `target_dir` holds a stamp at all.
///
/// Asked before a fingerprint is computed: a checkout with no stamp and no store
/// to look in has nothing to compare a key against, and resolving the compiler
/// to build one would be work spent on a question nobody asked.
pub(crate) fn is_stamped(target_dir: &Path) -> bool {
    target_dir.join(STAMP_FILE_NAME).is_file()
}

/// The rule host recorded by the stamp in `target_dir`, when it is still the one
/// `fingerprint` asks for and the bytes on disk are still the recorded bytes.
///
/// `None` is always "obtain the host some other way", never an error.
pub(crate) fn binary_recorded_by_stamp(target_dir: &Path, fingerprint: &str) -> Option<PathBuf> {
    let bytes = std::fs::read(target_dir.join(STAMP_FILE_NAME)).ok()?;
    let stamp = serde_json::from_slice::<HostStamp>(&bytes).ok()?;
    if stamp.schema != SCHEMA || stamp.fingerprint != fingerprint {
        return None;
    }
    let binary = target_dir.join(relative_target_path(&stamp.target_relative_path)?);
    let (len, sha256) = file_identity(&binary).ok()?;
    (len == stamp.len && sha256 == stamp.sha256).then_some(binary)
}

/// Record that `binary` is the rule host `fingerprint` names.
///
/// Best effort: a stamp that cannot be written only costs the next run the fast
/// path.
fn write_stamp(target_dir: &Path, fingerprint: &str, entry: &StoreEntry) {
    let stamp = HostStamp {
        schema: SCHEMA.to_string(),
        fingerprint: fingerprint.to_string(),
        target_relative_path: entry.target_relative_path.clone(),
        sha256: entry.sha256.clone(),
        len: entry.len,
    };
    if let Ok(bytes) = serde_json::to_vec(&stamp) {
        let _ = write_atomically(&target_dir.join(STAMP_FILE_NAME), &bytes);
    }
}

/// Copy the stored rule host for `fingerprint` into `target_dir`, or answer
/// `None`.
///
/// `None` is always "build it here instead", never an error: an absent entry, an
/// entry this polint cannot read, a missing blob, a copy that fails, or bytes
/// that do not hash to what the entry recorded all mean the same thing to the
/// caller. An entry that is *provably* wrong — the wrong schema, the wrong key,
/// the wrong polint version, or a blob that does not match its own record — is
/// deleted on the way out, because it can never become right.
pub(crate) fn restore(
    store: &RuleHostStore,
    target_dir: &Path,
    fingerprint: &str,
) -> Option<PathBuf> {
    let bytes = store.read(fingerprint)?;
    let entry = serde_json::from_slice::<StoreEntry>(&bytes)
        .ok()
        .filter(|entry| {
            entry.schema == SCHEMA
                && entry.key == fingerprint
                && entry.polint_version == env!("CARGO_PKG_VERSION")
        });
    let Some(entry) = entry else {
        store.discard(fingerprint);
        return None;
    };
    let Some(relative) = relative_target_path(&entry.target_relative_path) else {
        store.discard(fingerprint);
        return None;
    };
    let Some(blob) = store.blob_path(&entry.sha256) else {
        store.discard(fingerprint);
        return None;
    };

    // The binary is copied to a temporary name beside its destination and
    // verified BEFORE it is moved into place, so a corrupt entry never lands at
    // the path polint is about to execute. Renaming rather than writing over the
    // destination is also what makes replacing a host this machine is currently
    // running safe: on Unix the running process keeps its own inode, and on
    // Windows the rename fails instead of tearing the file, which is a miss like
    // any other.
    let destination = target_dir.join(relative);
    let parent = destination.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    let temporary = temporary_beside(&destination);
    let restored = std::fs::copy(&blob, &temporary)
        .ok()
        .and_then(|_| file_identity(&temporary).ok())
        .is_some_and(|(len, sha256)| len == entry.len && sha256 == entry.sha256);
    if !restored {
        let _ = std::fs::remove_file(&temporary);
        // The blob did not match the entry that named it, so neither is usable
        // by anyone.
        let _ = std::fs::remove_file(&blob);
        store.discard(fingerprint);
        return None;
    }
    if make_executable(&temporary).is_err() || std::fs::rename(&temporary, &destination).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return None;
    }
    write_stamp(target_dir, fingerprint, &entry);
    Some(destination)
}

/// Record a freshly built rule host: stamp it for this checkout, and publish it
/// to `store` for every other one.
///
/// Best effort in every half. The blob is written before the entry that names
/// it, so an entry never points at a blob that was never durable. Two publishers
/// of the same fingerprint can produce different bytes — cargo's output is not
/// byte-reproducible across target directories — so the blob is named by its own
/// content and the entry, not the blob, is what they race on. Either entry is
/// complete and correct.
pub(crate) fn record(
    store: Option<&RuleHostStore>,
    target_dir: &Path,
    fingerprint: &str,
    binary: &Path,
) {
    let Some(relative) = binary
        .strip_prefix(target_dir)
        .ok()
        .and_then(|relative| relative.to_str())
        .map(|relative| relative.replace('\\', "/"))
    else {
        return;
    };
    if relative_target_path(&relative).is_none() {
        return;
    }
    let Ok((len, sha256)) = file_identity(binary) else {
        return;
    };
    let entry = StoreEntry {
        schema: SCHEMA.to_string(),
        key: fingerprint.to_string(),
        polint_version: env!("CARGO_PKG_VERSION").to_string(),
        target_relative_path: relative,
        sha256,
        len,
    };
    write_stamp(target_dir, fingerprint, &entry);
    if let Some(store) = store {
        store.publish_blob(&entry.sha256, binary);
        if let Ok(bytes) = serde_json::to_vec(&entry) {
            store.publish(fingerprint, &bytes);
        }
    }
}

/// `recorded` as a path under a cargo target directory, or `None` when it is not
/// one.
///
/// The value is read from a file another process wrote, so it is validated
/// rather than trusted: a relative path with no `..` and no root is the only
/// shape that stays inside the directory polint pins.
fn relative_target_path(recorded: &str) -> Option<PathBuf> {
    if recorded.is_empty() {
        return None;
    }
    let path = Path::new(recorded);
    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then(|| path.to_path_buf())
}

/// The subdirectory a key or hash is filed under, or `None` when it is not a hex
/// digest.
///
/// Keys are validated here rather than trusted: a key reaches the store from a
/// hash function today, and restricting it to hex is what keeps that true no
/// matter what a future caller passes. It also keeps one directory from
/// collecting every entry a machine ever wrote.
fn shard(key: &str) -> Option<String> {
    if key.len() < 2 || key.len() > MAX_KEY_LEN || !key.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(key.get(..2)?.to_string())
}

/// Publish `bytes` at `path` so a reader sees either the whole file or no file.
///
/// The temporary carries the writer's pid, so two processes publishing at once
/// never collide, and the rename happens inside the destination directory, so it
/// is a rename rather than a copy.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(std::io::Error::other(
            "a store entry has no parent directory",
        ));
    };
    create_dir_all_private(parent)?;
    let temporary = temporary_beside(path);
    if let Err(err) = std::fs::write(&temporary, bytes) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(err);
    }
    Ok(())
}

/// A temporary name beside `path`, unique to this process.
fn temporary_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "entry".to_string(), |name| name.to_string_lossy().into());
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// Create `dir` and its parents, private to the invoking user.
///
/// polint owns the directories it creates under the store root, so it creates
/// them `0700`: the store holds a binary this machine will execute, and a
/// directory another local user can write into is a directory that decides what
/// runs. A directory that already exists keeps whatever mode it has — that one
/// is the user's own to choose.
#[cfg(unix)]
fn create_dir_all_private(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn create_dir_all_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Make a restored rule host runnable.
///
/// A copy carries the blob's mode, and the blob carries the mode of the binary
/// cargo produced, so this is normally already true; it is set anyway because a
/// store whose files arrived some other way — a restored backup, a copy through
/// a tool that dropped the bit — would otherwise produce a host that cannot be
/// executed at all.
#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Where the store lives, given what the environment says — a pure function of
/// its four readings.
///
/// Pure so the whole resolution matrix is testable without touching the process
/// environment, which is also what keeps the tests hermetic and safe to run in
/// parallel.
fn resolve_root(
    override_value: Option<&str>,
    xdg_cache_home: Option<&str>,
    local_app_data: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    if let Some(value) = override_value {
        let trimmed = value.trim();
        if OFF_VALUES
            .iter()
            .any(|off| trimmed.eq_ignore_ascii_case(off))
        {
            return None;
        }
        let path = Path::new(trimmed);
        // A relative store would follow the process's working directory, which
        // is not a location: the same command run from two directories would
        // mean two stores. Only an absolute path names one.
        return path.is_absolute().then(|| path.to_path_buf());
    }
    Some(
        platform_cache_dir(xdg_cache_home, local_app_data, home)?
            .join(CACHE_DIR_NAME)
            .join(STORE_DIR_NAME),
    )
}

/// The platform's user cache directory: `%LOCALAPPDATA%`, `~/Library/Caches`, or
/// `$XDG_CACHE_HOME`.
///
/// Each branch is the convention of the platform it names, taken as an idea
/// rather than through a dependency: a handful of `std::env` readings answer it,
/// and a directory layout is not a product fact worth another crate in the tree.
fn platform_cache_dir(
    xdg_cache_home: Option<&str>,
    local_app_data: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    if cfg!(windows) {
        return absolute(local_app_data);
    }
    if cfg!(target_os = "macos") {
        return Some(absolute(home)?.join("Library").join("Caches"));
    }
    // The XDG base directory specification says a relative $XDG_CACHE_HOME is
    // invalid and must be ignored, which lands on the same `~/.cache` the unset
    // case uses.
    absolute(xdg_cache_home).or_else(|| Some(absolute(home)?.join(".cache")))
}

/// `value` as a path, when it is a non-empty absolute one.
fn absolute(value: Option<&str>) -> Option<PathBuf> {
    let path = Path::new(value?.trim());
    (path.is_absolute()).then(|| path.to_path_buf())
}

/// The compiler and cargo a rule-host build would use, read once and passed in.
///
/// Resolving the compiler beats hashing `rust-toolchain.toml` alone: it also
/// covers a pinned toolchain override and a floating `stable` that moved. Both
/// readings are taken in the repository root, because that is where polint spawns
/// cargo and therefore where rustup resolves a toolchain file.
#[derive(Debug, Clone)]
pub(crate) struct ToolchainIdentity {
    /// The full `rustc -vV` output.
    pub(crate) rustc: String,
    /// The `cargo -V` output.
    pub(crate) cargo: String,
    /// The toolchain override in effect, if any.
    pub(crate) rustup_toolchain: Option<String>,
}

/// Everything about a rule-host build that is not a file: the compiler, the
/// profile, the target platform, and the flags cargo takes from its environment.
///
/// The environment is read once, when this is built, so that everything below it
/// is a function of its arguments — the same discipline [`resolve_root`] follows,
/// and what lets the fingerprint's behavior be tested without a process-wide
/// mutation that other tests would see.
#[derive(Debug, Clone)]
pub(crate) struct BuildEnvironment {
    /// The resolved cargo profile: `release`, `dev`, or a named profile.
    profile: String,
    toolchain: ToolchainIdentity,
    /// The value of each [`CARGO_FLAG_VARIABLES`] entry, in that order.
    cargo_flags: [String; CARGO_FLAG_VARIABLES.len()],
}

impl BuildEnvironment {
    /// The build environment of this process, for a host compiled under
    /// `profile` by `toolchain`.
    pub(crate) fn new(profile: String, toolchain: ToolchainIdentity) -> Self {
        Self {
            profile,
            toolchain,
            cargo_flags: CARGO_FLAG_VARIABLES
                .map(|(_, variable)| std::env::var(variable).unwrap_or_else(|_| UNSET.to_string())),
        }
    }
}

/// The two digests of a rule-host build's inputs.
///
/// A build is identified by everything it reads, but a build also WRITES one of
/// those inputs: cargo creates or updates the rule package's lockfile as part of
/// compiling it. The complete digest is therefore taken again once the build has
/// finished — that is the identity the host actually has, and the one every
/// later run will compute — while the authored digest, which leaves the
/// lockfiles out, brackets the build: unchanged across it means the sources
/// compiled are the sources being recorded, and a rule edited while cargo was
/// running is never published as if it had been compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildFingerprint {
    /// Every input, lockfiles included. This is the store key and what a stamp
    /// records.
    pub(crate) complete: String,
    /// Every input a build cannot rewrite for itself.
    pub(crate) authored: String,
}

/// Both digests over one input surface, folded in a single pass.
struct FingerprintDigest {
    complete: Sha256,
    authored: Sha256,
}

impl FingerprintDigest {
    fn new() -> Self {
        let mut digest = Self {
            complete: Sha256::new(),
            authored: Sha256::new(),
        };
        for hasher in [&mut digest.complete, &mut digest.authored] {
            hasher.update(SCHEMA.as_bytes());
            hasher.update(b"\n");
        }
        digest
    }

    /// One `key=value` line, terminated by a newline — the same shape the CI
    /// action's cache-input digests take.
    fn line(&mut self, key: &str, value: &str) {
        Self::write(&mut self.complete, key, value);
        Self::write(&mut self.authored, key, value);
    }

    /// A lockfile: an input to the build that the build may also write.
    fn lockfile_line(&mut self, key: &str, value: &str) {
        Self::write(&mut self.complete, key, value);
    }

    fn write(hasher: &mut Sha256, key: &str, value: &str) {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }

    fn finish(self) -> BuildFingerprint {
        BuildFingerprint {
            complete: hex(&self.complete.finalize()),
            authored: hex(&self.authored.finalize()),
        }
    }
}

/// The digests over every input a rule-host build reads, or `None` when the
/// surface could not be read.
///
/// Everything is hashed from bytes. A modification time says when a file was
/// written, not what is in it, and a restored checkout or a same-length rewrite
/// would pass as unchanged.
///
/// `None` means "no key for this build", which sends the caller to cargo. That
/// is the answer for every failure here, because a key computed over a surface
/// this run could not read would name a build it cannot stand for.
pub(crate) fn build_fingerprint(
    repo_root: &Path,
    rule_pkg_dir: &Path,
    environment: &BuildEnvironment,
) -> Option<BuildFingerprint> {
    let mut digest = FingerprintDigest::new();
    digest.line("polint", env!("CARGO_PKG_VERSION"));
    digest.line("profile", &environment.profile);
    digest.line("os", std::env::consts::OS);
    digest.line("arch", std::env::consts::ARCH);
    digest.line("rustc", &environment.toolchain.rustc);
    digest.line("cargo", &environment.toolchain.cargo);
    digest.line(
        "rustup_toolchain",
        environment
            .toolchain
            .rustup_toolchain
            .as_deref()
            .unwrap_or(UNSET),
    );
    for ((key, _), value) in CARGO_FLAG_VARIABLES.iter().zip(&environment.cargo_flags) {
        digest.line(key, value);
    }

    // The files cargo and rustup discover from the directory polint spawns cargo
    // in, and then the rule package's own. Both lockfiles matter: the one beside
    // the manifest cargo actually uses pins every registry and git dependency by
    // checksum, which is what lets a digest over these files stand for the whole
    // of what the build compiles.
    digest.line(
        "repo_cargo_toml",
        &optional_file_digest(&repo_root.join("Cargo.toml"))?,
    );
    digest.lockfile_line(
        "repo_cargo_lock",
        &optional_file_digest(&repo_root.join("Cargo.lock"))?,
    );
    for (key, relative) in [
        ("rust_toolchain_toml", "rust-toolchain.toml"),
        ("cargo_config_toml", ".cargo/config.toml"),
        ("cargo_config", ".cargo/config"),
    ] {
        digest.line(key, &optional_file_digest(&repo_root.join(relative))?);
    }
    digest.line("package", &package_label(repo_root, rule_pkg_dir));
    digest.line(
        "package_cargo_toml",
        &optional_file_digest(&rule_pkg_dir.join("Cargo.toml"))?,
    );
    digest.lockfile_line(
        "package_cargo_lock",
        &optional_file_digest(&rule_pkg_dir.join("Cargo.lock"))?,
    );
    for (relative, source) in rule_sources(rule_pkg_dir)? {
        digest.line(&relative, &source);
    }
    digest.line(
        "bin",
        &manifest_bin_names(&rule_pkg_dir.join("Cargo.toml"))?,
    );

    Some(digest.finish())
}

/// How the rule package is named in a fingerprint: its path relative to the
/// repository root, or the path as given when it is somewhere else entirely.
fn package_label(repo_root: &Path, rule_pkg_dir: &Path) -> String {
    rule_pkg_dir
        .strip_prefix(repo_root)
        .unwrap_or(rule_pkg_dir)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every Rust source in the package, as `<package-relative path>` and digest
/// pairs ordered by path.
///
/// The whole package is walked rather than just `src/`, because `src/` is a
/// convention and not a boundary: a `[[bin]] path` may name a file anywhere in
/// the package, and a `build.rs` beside the manifest runs in the compiler and
/// decides what the crate becomes. A key that stands in for cargo's own
/// freshness check has to cover both. Files a binary does not compile — a
/// `tests/` directory, say — cost a rebuild when they change and can never cause
/// a stale host to run.
///
/// `None` when a directory or a file that is there cannot be read: the set would
/// then describe fewer sources than the build compiles.
fn rule_sources(rule_pkg_dir: &Path) -> Option<Vec<(String, String)>> {
    let files = package_files(rule_pkg_dir, &|path| {
        path.extension().is_some_and(|extension| extension == "rs")
    })?;
    let mut out = Vec::with_capacity(files.len());
    for path in files {
        let relative = path
            .strip_prefix(rule_pkg_dir)
            .ok()?
            .to_str()?
            .replace('\\', "/");
        out.push((relative, file_digest(&path).ok()?));
    }
    // Byte order over the package-relative path, which is the same order the CI
    // action's `LC_ALL=C sort` produces for the same set.
    out.sort();
    Some(out)
}

/// Every file under the rule package that `keep` accepts.
///
/// `target/` at the package root is cargo's own output rather than a source, and
/// symlinked directories are not followed: a walk that leaves the package cannot
/// answer a question about what is inside it. `None` when a directory cannot be
/// read, because a partial walk describes fewer inputs than the build has.
///
/// One walk with one set of exclusions serves both questions asked of a rule
/// package — what it compiles, and where its manifests point — so the two can
/// never disagree about what "in the package" means.
fn package_files(rule_pkg_dir: &Path, keep: &dyn Fn(&Path) -> bool) -> Option<Vec<PathBuf>> {
    fn collect(
        root: &Path,
        dir: &Path,
        keep: &dyn Fn(&Path) -> bool,
        out: &mut Vec<PathBuf>,
    ) -> Option<()> {
        for entry in std::fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if dir == root && path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                collect(root, &path, keep, out)?;
            } else if keep(&path) {
                out.push(path);
            }
        }
        Some(())
    }

    let mut out = Vec::new();
    collect(rule_pkg_dir, rule_pkg_dir, keep, &mut out)?;
    Some(out)
}

/// The binary targets a manifest declares, sorted and comma-joined.
///
/// A package with no `[[bin]]` table builds `src/main.rs` under the package
/// name, so that is the answer there. `None` when the manifest cannot be read or
/// parsed, which is a build that will fail for its own reasons.
fn manifest_bin_names(manifest: &Path) -> Option<String> {
    let value = std::fs::read_to_string(manifest)
        .ok()
        .and_then(|text| toml::from_str::<toml::Value>(&text).ok())?;
    let mut names = value
        .get("bin")
        .and_then(toml::Value::as_array)
        .map(|bins| {
            bins.iter()
                .filter_map(|bin| bin.get("name")?.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if names.is_empty() {
        names = value
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .map(|name| vec![name.to_string()])?;
    }
    names.sort();
    Some(names.join(","))
}

/// The digest of a file that may legitimately not exist.
///
/// A file that is not there is recorded as absent, which is a value like any
/// other. A file that IS there and cannot be read answers `None`, because
/// recording it as absent would give a surface polint could not read the same
/// name as one that does not have it.
fn optional_file_digest(path: &Path) -> Option<String> {
    match file_digest(path) {
        Ok(digest) => Some(digest),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(ABSENT.to_string()),
        Err(_) => None,
    }
}

/// The sha256 of a file's bytes.
fn file_digest(path: &Path) -> std::io::Result<String> {
    Ok(file_identity(path)?.1)
}

/// A file's length and the sha256 of its bytes, read in one streaming pass so a
/// rule host of any size costs one buffer rather than its own size in memory.
fn file_identity(path: &Path) -> std::io::Result<(u64, String)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut len = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        len += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((len, hex(&hasher.finalize())))
}

/// Lowercase hex, the form every key, digest, and blob name in the store takes.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Whether a fingerprint over this rule package names the same bytes read from
/// any checkout on this machine.
///
/// The fingerprint hashes the package's manifests, lockfiles, and sources, which
/// is the whole of what the build compiles as long as every dependency comes
/// from a registry or a git revision: the lockfile pins those and cargo verifies
/// them by checksum. Two constructs break that.
///
/// A `path` dependency is what a manifest has for naming sources the fingerprint
/// cannot see, and a RELATIVE one is resolved against the manifest — so
/// `../helpers` names a different directory in every checkout, and two checkouts
/// holding byte-identical rule sources would compute one fingerprint over two
/// different builds. An absolute path names one directory on this machine, and a
/// path that stays inside the package is hashed by content like every other
/// input; both mean the fingerprint identifies the same bytes wherever it is
/// computed.
///
/// A cargo config does it from outside the tree altogether: `[patch]`,
/// `[replace]` and `paths` send a build to sources no manifest and no lockfile
/// records, so the package is byte-identical either side of such a config while
/// what it compiles is not.
///
/// Anything unreadable or unparseable answers `false`: a surface that cannot be
/// proven is never shared. A false yes costs a local build; a false no shares the
/// wrong binary.
pub(crate) fn inputs_are_the_same_from_every_checkout(
    rule_pkg_dir: &Path,
    repo_root: &Path,
) -> bool {
    shareable_with_cargo_home(rule_pkg_dir, repo_root, cargo_home())
}

/// [`inputs_are_the_same_from_every_checkout`] with cargo's home directory given
/// rather than read, so the answer is a function of its arguments.
fn shareable_with_cargo_home(
    rule_pkg_dir: &Path,
    repo_root: &Path,
    cargo_home: Option<PathBuf>,
) -> bool {
    every_manifest_path_stays_put(rule_pkg_dir)
        && !a_cargo_config_redirects_a_dependency(rule_pkg_dir, repo_root, cargo_home)
}

/// Whether every `path` declared by every manifest under the rule package names
/// the same directory from every checkout.
///
/// Every manifest is read, not only the package's own: a `path` dependency can
/// point at a directory inside the package that has a manifest of its own, and
/// that manifest's pointers leave the package just as easily.
fn every_manifest_path_stays_put(rule_pkg_dir: &Path) -> bool {
    let manifests = package_files(rule_pkg_dir, &|path| {
        path.file_name().is_some_and(|name| name == "Cargo.toml")
    });
    manifests.is_some_and(|manifests| {
        manifests
            .iter()
            .all(|manifest| manifest_paths_stay_put(rule_pkg_dir, manifest))
    })
}

/// Whether every `path` one manifest declares names the same directory from
/// every checkout.
fn manifest_paths_stay_put(rule_pkg_dir: &Path, manifest: &Path) -> bool {
    let Some(manifest_dir) = manifest.parent() else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&text) else {
        return false;
    };
    let mut declared = Vec::new();
    collect_declared_paths(&value, &mut declared);
    declared.iter().all(|path| {
        let path = Path::new(path);
        path.is_absolute() || resolved_lexically(manifest_dir, path).starts_with(rule_pkg_dir)
    })
}

/// Every string filed under a `path` key anywhere in a manifest.
///
/// Taken over the whole document rather than a list of dependency tables,
/// because a manifest has many of them — `[dependencies]`, `[dev-dependencies]`,
/// `[build-dependencies]`, one pair per `[target.'cfg(…)']`, `[patch.*]`,
/// `[workspace.dependencies]` — and a table this misses is a pointer that leaves
/// the fingerprint. The keys it also collects (`[[bin]] path`, `[lib] path`) name
/// files inside the package, which is the answer they should give anyway.
fn collect_declared_paths(value: &toml::Value, out: &mut Vec<String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, nested) in table {
                if key == "path"
                    && let Some(text) = nested.as_str()
                {
                    out.push(text.to_string());
                }
                collect_declared_paths(nested, out);
            }
        }
        toml::Value::Array(items) => {
            for item in items {
                collect_declared_paths(item, out);
            }
        }
        _ => {}
    }
}

/// `path` joined to `base` and reduced without touching the filesystem.
///
/// Lexical rather than canonical because the question is where the manifest
/// points, not where a symlink would land, and because the directory need not
/// exist for the answer to be knowable.
fn resolved_lexically(base: &Path, path: &Path) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Whether a cargo config that applies to this build redirects where a
/// dependency's source is read from.
///
/// It never asks WHICH package is redirected. A `paths` entry names directories
/// rather than packages and could only be attributed by reading the manifests it
/// points at, a `replace` key is a package-id spec that would have to be parsed
/// to be attributed, and a patch of any crate in the host's graph moves bytes the
/// fingerprint cannot see exactly as a patch of `polint` does. One question —
/// does this build read sources the fingerprint cannot name — has one answer for
/// all of them.
fn a_cargo_config_redirects_a_dependency(
    rule_pkg_dir: &Path,
    repo_root: &Path,
    cargo_home: Option<PathBuf>,
) -> bool {
    cargo_config_files(rule_pkg_dir, repo_root, cargo_home)
        .iter()
        .any(|path| redirects_a_dependency(path))
}

/// Every cargo config file that can apply to a rule-host build.
///
/// polint spawns cargo with the repository root as its working directory, and
/// cargo merges the `.cargo/` config of that directory with those of every
/// ancestor and with `$CARGO_HOME`'s. The rule package's own is read too, for a
/// cargo run from inside it. Both file names are taken at every location because
/// cargo still honors the extensionless one it used before 1.39. Nothing here
/// requires a file to exist; a name that is not there is read as absent.
fn cargo_config_files(
    rule_pkg_dir: &Path,
    repo_root: &Path,
    cargo_home: Option<PathBuf>,
) -> Vec<PathBuf> {
    std::iter::once(rule_pkg_dir)
        .chain(repo_root.ancestors())
        .map(|dir| dir.join(".cargo"))
        .chain(cargo_home)
        .flat_map(|dir| CARGO_CONFIG_NAMES.map(|name| dir.join(name)))
        .collect()
}

/// Where cargo keeps its user-wide config: `$CARGO_HOME`, else `~/.cargo`,
/// resolved the way cargo itself resolves it. `None` when the environment names
/// neither, which is no location to read.
fn cargo_home() -> Option<PathBuf> {
    if let Some(home) = env_path("CARGO_HOME") {
        return Some(home);
    }
    env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .map(|home| home.join(".cargo"))
}

/// One environment variable read as a directory, treating an empty value as the
/// absence it means.
fn env_path(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Whether one cargo config file redirects where a package's source is read
/// from.
///
/// A file that is not there redirects nothing — nor does a location where cargo
/// could not keep one, such as a `.cargo` that is a file rather than a directory.
/// A file that IS there and cannot be read or parsed is treated as if it did
/// redirect: the store may only ever make a run faster, so a config whose effect
/// this run could not establish is never one it claims to have proven.
fn redirects_a_dependency(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return true;
    };
    let Ok(config) = toml::from_str::<toml::Value>(&text) else {
        return true;
    };
    CARGO_REDIRECT_KEYS
        .iter()
        .any(|key| config.get(key).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("polint-rules-store-{label}-"))
            .tempdir()
            .expect("create a temporary directory")
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directory");
        }
        std::fs::write(path, contents).expect("write fixture file");
    }

    fn release_environment() -> BuildEnvironment {
        BuildEnvironment {
            profile: "release".to_string(),
            toolchain: ToolchainIdentity {
                rustc: "rustc 1.95.0 (abcdef 2026-01-01)".to_string(),
                cargo: "cargo 1.95.0 (abcdef 2026-01-01)".to_string(),
                rustup_toolchain: None,
            },
            cargo_flags: CARGO_FLAG_VARIABLES.map(|_| UNSET.to_string()),
        }
    }

    /// The same environment with one compiler flag variable set.
    fn environment_with(key: &str, value: &str) -> BuildEnvironment {
        let index = CARGO_FLAG_VARIABLES
            .iter()
            .position(|(name, _)| *name == key)
            .expect("a fingerprint line this module records");
        let mut environment = release_environment();
        environment.cargo_flags[index] = value.to_string();
        environment
    }

    /// Every shareability question in this module is asked with cargo's home
    /// given rather than read, so a config in the developer's own `~/.cargo`
    /// cannot decide a test.
    fn shareable(rule_pkg_dir: &Path, repo_root: &Path) -> bool {
        shareable_with_cargo_home(rule_pkg_dir, repo_root, None)
    }

    /// A repository with one rule package that depends only on a registry crate.
    fn rule_package(root: &Path) -> PathBuf {
        let package = root.join(".polint/rules");
        write(
            &package.join("Cargo.toml"),
            "[package]\nname = \"polint-local-rules\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\npolint = \"0.3.1\"\n\n[workspace]\n",
        );
        write(&package.join("src/main.rs"), "fn main() {}\n");
        package
    }

    #[test]
    fn an_absolute_override_names_the_store_and_off_turns_sharing_off() {
        assert_eq!(
            resolve_root(Some("/tmp/shared"), None, None, Some("/home/u")),
            Some(PathBuf::from("/tmp/shared"))
        );
        for off in ["off", "OFF", "  Disabled ", "none", "None"] {
            assert_eq!(
                resolve_root(Some(off), None, None, Some("/home/u")),
                None,
                "{off} must turn sharing off"
            );
        }
    }

    #[test]
    fn a_value_that_is_not_a_location_turns_sharing_off_rather_than_guessing() {
        for value in ["", "   ", "relative/store", "./store", "~/store"] {
            assert_eq!(
                resolve_root(Some(value), None, None, Some("/home/u")),
                None,
                "{value:?} is not an absolute path and must not resolve to a store"
            );
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn the_default_follows_the_xdg_base_directory_specification() {
        assert_eq!(
            resolve_root(None, Some("/x/cache"), None, Some("/home/u")),
            Some(PathBuf::from("/x/cache/polint/store"))
        );
        assert_eq!(
            resolve_root(None, None, None, Some("/home/u")),
            Some(PathBuf::from("/home/u/.cache/polint/store"))
        );
        // A relative XDG_CACHE_HOME is invalid per the specification and is ignored.
        assert_eq!(
            resolve_root(None, Some("relative"), None, Some("/home/u")),
            Some(PathBuf::from("/home/u/.cache/polint/store"))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_default_is_the_user_caches_directory() {
        assert_eq!(
            resolve_root(None, Some("/x/cache"), None, Some("/home/u")),
            Some(PathBuf::from("/home/u/Library/Caches/polint/store"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_default_is_the_local_application_data_directory() {
        assert_eq!(
            resolve_root(None, None, Some(r"C:\Users\u\AppData\Local"), None),
            Some(PathBuf::from(r"C:\Users\u\AppData\Local\polint\store"))
        );
    }

    #[test]
    fn an_environment_with_no_home_shares_nothing_instead_of_failing() {
        assert_eq!(resolve_root(None, None, None, None), None);
    }

    #[test]
    fn a_key_that_is_not_a_hex_digest_is_not_a_key() {
        assert_eq!(shard("../../etc"), None);
        assert_eq!(shard("a/b"), None);
        assert_eq!(shard(""), None);
        assert_eq!(shard("a"), None);
        assert_eq!(shard(&"a".repeat(129)), None);
        assert_eq!(shard("deadbeef"), Some("de".to_string()));
        assert_eq!(shard("DEADBEEF"), Some("DE".to_string()));
    }

    #[test]
    fn a_recorded_binary_path_may_not_leave_the_target_directory() {
        for escape in [
            "",
            "..",
            "../outside",
            "release/../../outside",
            "/etc/passwd",
        ] {
            assert_eq!(
                relative_target_path(escape),
                None,
                "{escape:?} must not resolve under the target directory"
            );
        }
        assert_eq!(
            relative_target_path("release/polint-local-rules"),
            Some(PathBuf::from("release/polint-local-rules"))
        );
    }

    #[test]
    fn a_fingerprint_is_stable_and_changes_with_every_input_it_names() {
        let temp = temp_dir("fingerprint");
        let root = temp.path();
        let package = rule_package(root);
        let environment = release_environment();

        let baseline = build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_eq!(
            build_fingerprint(root, &package, &environment),
            Some(baseline.clone()),
            "the same inputs must produce the same key"
        );
        assert_eq!(baseline.complete.len(), 64, "a key is a sha256 hex digest");
        assert_ne!(
            baseline.complete, baseline.authored,
            "the two digests cover different input sets"
        );

        // A lockfile is an input a build may write for itself, so it moves the
        // key a later run computes without moving the bracket that says the
        // sources compiled were the sources recorded.
        write(&package.join("Cargo.lock"), "version = 4\n");
        let locked = build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_ne!(
            baseline.complete, locked.complete,
            "a lockfile is part of the identity a build has"
        );
        assert_eq!(
            baseline.authored, locked.authored,
            "a lockfile cargo wrote is not a change to the authored sources"
        );
        std::fs::remove_file(package.join("Cargo.lock")).expect("remove the lockfile");

        write(&package.join("src/main.rs"), "fn main() { }\n");
        let changed_source =
            build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_ne!(
            baseline, changed_source,
            "a changed rule source is a new key"
        );

        write(
            &package.join("Cargo.toml"),
            "[package]\nname = \"polint-local-rules\"\nversion = \"0.0.1\"\nedition = \"2024\"\n\n\
             [dependencies]\npolint = \"0.3.1\"\n\n[workspace]\n",
        );
        let changed_manifest =
            build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_ne!(
            changed_source, changed_manifest,
            "a changed manifest is a new key"
        );

        let mut dev = release_environment();
        dev.profile = "dev".to_string();
        assert_ne!(
            build_fingerprint(root, &package, &dev),
            Some(changed_manifest.clone()),
            "a different cargo profile is a new key"
        );

        let mut other_compiler = release_environment();
        other_compiler.toolchain.rustc = "rustc 1.96.0 (fedcba 2026-02-02)".to_string();
        assert_ne!(
            build_fingerprint(root, &package, &other_compiler),
            Some(changed_manifest.clone()),
            "a different compiler is a new key"
        );

        // RUSTFLAGS changes what the compiler produces from unchanged sources,
        // so it has to change the key.
        let with_flags = build_fingerprint(
            root,
            &package,
            &environment_with("rustflags", "-C target-cpu=native"),
        );
        assert_ne!(
            with_flags,
            Some(changed_manifest),
            "RUSTFLAGS is part of the key"
        );
    }

    #[test]
    fn every_rust_source_in_the_package_is_part_of_the_key() {
        let temp = temp_dir("sources");
        let root = temp.path();
        let package = rule_package(root);
        let environment = release_environment();

        let baseline = build_fingerprint(root, &package, &environment).expect("a fingerprint");
        write(&package.join("src/rules/extra.rs"), "// a new rule\n");
        let nested = build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_ne!(
            baseline, nested,
            "a rule source added in a subdirectory is a new key"
        );

        // A build script runs in the compiler and decides what the crate
        // becomes; it lives beside the manifest rather than under `src/`.
        write(&package.join("build.rs"), "fn main() {}\n");
        let with_build_script =
            build_fingerprint(root, &package, &environment).expect("a fingerprint");
        assert_ne!(
            nested, with_build_script,
            "a build script is part of what the package compiles to"
        );

        // Cargo's own output under the package is not a source.
        write(&package.join("target/debug/build/stale.rs"), "// output\n");
        assert_eq!(
            build_fingerprint(root, &package, &environment),
            Some(with_build_script),
            "a package-local cargo target directory is output, not input"
        );
    }

    #[test]
    fn a_package_that_is_not_there_has_no_key() {
        let temp = temp_dir("missing");
        assert_eq!(
            build_fingerprint(
                temp.path(),
                &temp.path().join("nowhere"),
                &release_environment()
            ),
            None
        );
    }

    #[test]
    fn a_registry_only_rule_package_is_the_same_from_every_checkout() {
        let temp = temp_dir("shareable");
        let root = temp.path();
        let package = rule_package(root);
        assert!(shareable(&package, root));
    }

    #[test]
    fn a_relative_path_dependency_that_leaves_the_package_is_not_shareable() {
        let temp = temp_dir("escaping");
        let root = temp.path();
        let package = rule_package(root);
        write(
            &package.join("Cargo.toml"),
            "[package]\nname = \"polint-local-rules\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nhelpers = { path = \"../../helpers\" }\n\n[workspace]\n",
        );
        assert!(!shareable(&package, root));
    }

    #[test]
    fn an_absolute_path_dependency_names_one_directory_and_is_shareable() {
        let temp = temp_dir("absolute");
        let root = temp.path();
        let package = rule_package(root);
        let elsewhere = root.join("elsewhere").to_string_lossy().replace('\\', "/");
        write(
            &package.join("Cargo.toml"),
            &format!(
                "[package]\nname = \"polint-local-rules\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
                 [dependencies]\npolint = {{ path = \"{elsewhere}\" }}\n\n[workspace]\n"
            ),
        );
        assert!(shareable(&package, root));
    }

    #[test]
    fn a_path_dependency_inside_the_package_is_shareable() {
        let temp = temp_dir("inside");
        let root = temp.path();
        let package = rule_package(root);
        write(
            &package.join("Cargo.toml"),
            "[package]\nname = \"polint-local-rules\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nhelpers = { path = \"helpers\" }\n\n[workspace]\n",
        );
        write(
            &package.join("helpers/Cargo.toml"),
            "[package]\nname = \"helpers\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        );
        assert!(shareable(&package, root));

        // A manifest nested inside the package is read too, so a pointer that
        // leaves from there is caught as well.
        write(
            &package.join("helpers/Cargo.toml"),
            "[package]\nname = \"helpers\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nfurther = { path = \"../../../elsewhere\" }\n",
        );
        assert!(!shareable(&package, root));
    }

    #[test]
    fn a_cargo_config_that_redirects_a_dependency_is_not_shareable() {
        for redirect in [
            "[patch.crates-io]\npolint = { path = \"/tmp/polint\" }\n",
            "[replace]\n\"polint:0.3.1\" = { path = \"/tmp/polint\" }\n",
            "paths = [\"/tmp/polint\"]\n",
            "include = \"other.toml\"\n",
        ] {
            let temp = temp_dir("redirect");
            let root = temp.path();
            let package = rule_package(root);
            write(&root.join(".cargo/config.toml"), redirect);
            assert!(
                !shareable(&package, root),
                "a config declaring {redirect:?} must not be shared"
            );
        }
    }

    #[test]
    fn a_cargo_config_that_cannot_be_parsed_is_not_shareable() {
        let temp = temp_dir("unparseable");
        let root = temp.path();
        let package = rule_package(root);
        write(&root.join(".cargo/config"), "this is not = = toml\n");
        assert!(!shareable(&package, root));
    }

    #[cfg(unix)]
    #[test]
    fn a_cargo_config_that_cannot_be_read_is_not_shareable() {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir("unreadable");
        let root = temp.path();
        let package = rule_package(root);
        let config = root.join(".cargo/config.toml");
        write(&config, "[build]\njobs = 1\n");
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000))
            .expect("drop read permission");
        // A process with the privilege to read the file regardless of its mode
        // has nothing to answer here; ask the question only where it applies.
        let denied = std::fs::read_to_string(&config).is_err();
        let answer = shareable(&package, root);
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644))
            .expect("restore read permission");
        if denied {
            assert!(!answer, "a config this run could not read is not proven");
        }
    }

    #[test]
    fn a_cargo_config_without_a_redirect_is_shareable() {
        let temp = temp_dir("plain-config");
        let root = temp.path();
        let package = rule_package(root);
        write(&root.join(".cargo/config.toml"), "[build]\njobs = 2\n");
        assert!(!a_cargo_config_redirects_a_dependency(&package, root, None));
    }

    #[test]
    fn a_published_host_is_restored_into_a_fresh_target_directory() {
        let temp = temp_dir("roundtrip");
        let store = RuleHostStore::at(temp.path().join("store"));
        let built = temp.path().join("built");
        let binary = built.join("release/polint-local-rules");
        write(&binary, "the compiled rule host\n");

        record(Some(&store), &built, &"ab".repeat(32), &binary);
        assert!(
            binary_recorded_by_stamp(&built, &"ab".repeat(32)).is_some(),
            "the build that published is stamped for its own checkout"
        );

        let restored_into = temp.path().join("restored");
        let restored =
            restore(&store, &restored_into, &"ab".repeat(32)).expect("the stored host is restored");
        assert_eq!(restored, restored_into.join("release/polint-local-rules"));
        assert_eq!(
            std::fs::read_to_string(&restored).expect("read the restored host"),
            "the compiled rule host\n"
        );
        assert_eq!(
            binary_recorded_by_stamp(&restored_into, &"ab".repeat(32)).as_deref(),
            Some(restored.as_path()),
            "a restore stamps the checkout it restored into"
        );
    }

    #[test]
    fn an_entry_for_another_question_is_never_offered_as_the_answer() {
        let temp = temp_dir("wrong-key");
        let store = RuleHostStore::at(temp.path().join("store"));
        let built = temp.path().join("built");
        let binary = built.join("release/polint-local-rules");
        write(&binary, "the compiled rule host\n");
        record(Some(&store), &built, &"ab".repeat(32), &binary);

        // The same bytes, filed under a key that is not the one recorded inside.
        let recorded = store.read(&"ab".repeat(32)).expect("the published entry");
        store.publish(&"cd".repeat(32), &recorded);
        assert_eq!(
            restore(&store, &temp.path().join("restored"), &"cd".repeat(32)),
            None
        );
        assert_eq!(
            store.read(&"cd".repeat(32)),
            None,
            "an entry that cannot prove which question it answers is deleted"
        );
    }

    #[test]
    fn a_corrupt_blob_is_discarded_rather_than_executed() {
        let temp = temp_dir("corrupt");
        let store = RuleHostStore::at(temp.path().join("store"));
        let built = temp.path().join("built");
        let binary = built.join("release/polint-local-rules");
        write(&binary, "the compiled rule host\n");
        record(Some(&store), &built, &"ab".repeat(32), &binary);

        let entry: StoreEntry =
            serde_json::from_slice(&store.read(&"ab".repeat(32)).expect("the published entry"))
                .expect("a readable entry");
        let blob = store.blob_path(&entry.sha256).expect("a blob path");
        std::fs::write(&blob, "something else entirely\n").expect("corrupt the blob");

        let restored_into = temp.path().join("restored");
        assert_eq!(restore(&store, &restored_into, &"ab".repeat(32)), None);
        assert!(!blob.exists(), "a blob that is not its own name is removed");
        assert_eq!(
            store.read(&"ab".repeat(32)),
            None,
            "the entry that named it is removed with it"
        );
        assert!(
            !restored_into.join("release/polint-local-rules").exists(),
            "no unverified bytes are ever moved into place"
        );
    }

    #[test]
    fn a_stamp_stops_matching_when_the_binary_beside_it_changes() {
        let temp = temp_dir("stamp");
        let built = temp.path().join("built");
        let binary = built.join("release/polint-local-rules");
        write(&binary, "the compiled rule host\n");
        record(None, &built, &"ab".repeat(32), &binary);

        assert!(binary_recorded_by_stamp(&built, &"ab".repeat(32)).is_some());
        assert_eq!(
            binary_recorded_by_stamp(&built, &"cd".repeat(32)),
            None,
            "a stamp answers only the fingerprint it recorded"
        );

        write(&binary, "a different rule host\n");
        assert_eq!(
            binary_recorded_by_stamp(&built, &"ab".repeat(32)),
            None,
            "a replaced binary is not the one the stamp recorded"
        );

        std::fs::remove_file(&binary).expect("remove the host");
        assert_eq!(binary_recorded_by_stamp(&built, &"ab".repeat(32)), None);
    }

    #[test]
    fn publishing_never_fails_a_run_even_when_the_store_cannot_be_written() {
        let temp = temp_dir("unwritable");
        // A file where the store root must be a directory: every path under it is
        // unwritable.
        let root = temp.path().join("root");
        std::fs::write(&root, b"not a directory").expect("write the blocking file");
        let store = RuleHostStore::at(&root);
        let built = temp.path().join("built");
        let binary = built.join("release/polint-local-rules");
        write(&binary, "the compiled rule host\n");

        record(Some(&store), &built, &"ab".repeat(32), &binary);
        assert_eq!(
            restore(&store, &temp.path().join("restored"), &"ab".repeat(32)),
            None
        );
        assert!(
            binary_recorded_by_stamp(&built, &"ab".repeat(32)).is_some(),
            "a store that cannot be written still leaves this checkout stamped"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_directories_polint_creates_are_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir("private");
        let store = RuleHostStore::at(temp.path().join("store"));
        store.publish(&"ab".repeat(32), b"recorded");
        let mode = std::fs::metadata(temp.path().join("store"))
            .expect("the store root exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "the store root must be user-private");
    }
}
