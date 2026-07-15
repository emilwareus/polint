use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest as ShaDigest, Sha256};

use crate::go::semantic::protocol::GO_SEMANTIC_SCHEMA;

pub(crate) const GO_SEMANTIC_FRONTEND_ENV: &str = "POLINT_GO_FRONTEND";
const GO_FRONTEND_CACHE_VERSION: &str = "v1";
const GO_ENVIRONMENT_POLICY: &str = "host-target-clean-env-v1";
const GO_FRONTEND_MAX_SOURCE_FILES: usize = 512;
const GO_FRONTEND_MAX_SOURCE_BYTES: usize = 32 * 1_048_576;
const GO_FRONTEND_MAX_SOURCE_ENTRIES: usize = 4_096;
const GO_FRONTEND_MAX_SOURCE_DIRECTORIES: usize = 512;
const GO_FRONTEND_MAX_SOURCE_DEPTH: usize = 64;
const GO_FRONTEND_MAX_SOURCE_FRONTIER: usize = 512;
const GO_FRONTEND_MAX_EXECUTABLE_BYTES: usize = 64 * 1_048_576;
const GO_FRONTEND_MAX_STALE_STAGING_SCAN: usize = 64;
const GO_FRONTEND_STALE_STAGING_AGE: std::time::Duration =
    std::time::Duration::from_secs(24 * 60 * 60);
const GO_ENVIRONMENT_MODIFIERS: &[&str] = &[
    "GO386",
    "GOAMD64",
    "GOARCH",
    "GOARM",
    "GOARM64",
    "GOFIPS140",
    "CGO_ENABLED",
    "GODEBUG",
    "GOENV",
    "GOEXPERIMENT",
    "GOFLAGS",
    "GOMIPS",
    "GOMIPS64",
    "GOOS",
    "GOPPC64",
    "GORISCV64",
    "GOROOT",
    "GOTOOLCHAIN",
    "GOWORK",
    "GOWASM",
];
const EMBEDDED_GO_FRONTEND_FILES: &[(&str, &str)] = &[
    (
        "go.mod",
        include_str!("../../../go-sidecar/polint-go-frontend/go.mod"),
    ),
    (
        "go.sum",
        include_str!("../../../go-sidecar/polint-go-frontend/go.sum"),
    ),
    (
        "main.go",
        include_str!("../../../go-sidecar/polint-go-frontend/main.go"),
    ),
    (
        "internal/semantic/emit.go",
        include_str!("../../../go-sidecar/polint-go-frontend/internal/semantic/emit.go"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoSemanticProcessError {
    CommandFailed(String),
    CommandUnavailable(String),
    VersionUnsupported(String),
    Timeout(String),
}

impl std::fmt::Display for GoSemanticProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandFailed(reason) => write!(f, "{reason}"),
            Self::CommandUnavailable(reason) => write!(f, "{reason}"),
            Self::VersionUnsupported(reason) => write!(f, "{reason}"),
            Self::Timeout(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for GoSemanticProcessError {}

impl GoSemanticProcessError {
    pub(crate) fn stable_reason(&self) -> &'static str {
        match self {
            Self::CommandFailed(_) => "command_failed",
            Self::CommandUnavailable(_) => "command_unavailable",
            Self::VersionUnsupported(_) => "version_unsupported",
            Self::Timeout(_) => "timeout",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticCommand {
    Binary(PathBuf),
    SourceDir(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGoSemanticFrontend {
    executable: PathBuf,
    executable_digest: String,
    source_digest: Option<String>,
    toolchain: Box<PreparedGoToolchain>,
    environment_policy: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoHostTarget {
    os: String,
    arch: String,
}

impl GoHostTarget {
    fn current_process() -> Result<Self, GoSemanticProcessError> {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            other => {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "unsupported host operating system for Go semantic analysis: {other}"
                )));
            }
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "386",
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => {
                return Err(GoSemanticProcessError::CommandUnavailable(format!(
                    "unsupported host architecture for Go semantic analysis: {other}"
                )));
            }
        };
        Ok(Self {
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    fn parse_label(value: &str) -> Option<Self> {
        let (os, arch) = value.trim().split_once('/')?;
        if os.is_empty() || arch.is_empty() {
            return None;
        }
        Some(Self {
            os: os.to_string(),
            arch: arch.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}/{}", self.os, self.arch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoToolchain {
    executable: PathBuf,
    executable_digest: String,
    canonical_selection: PathBuf,
    version: String,
    host_target: GoHostTarget,
    goroot: PathBuf,
    search_path: OsString,
}

#[derive(Debug, Clone)]
struct PreparedGoToolchain {
    executable: PathBuf,
    executable_digest: String,
    canonical_selection: PathBuf,
    version: String,
    host_target: GoHostTarget,
    goroot: PathBuf,
    search_path: OsString,
    runtime_search_path: OsString,
}

#[derive(Debug, Clone)]
struct FrontendSourceFile {
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct FrontendSourceSnapshot {
    digest: String,
    files: Vec<FrontendSourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontendBuildProvenance {
    source_digest: String,
    toolchain_version: String,
    toolchain_executable_digest: String,
    toolchain_selection: String,
    goroot: String,
    search_path: String,
    host_target: GoHostTarget,
    environment_policy: &'static str,
}

impl FrontendBuildProvenance {
    fn cache_key(&self) -> String {
        security_digest_strings(&[
            format!("source_digest={}", self.source_digest),
            format!("toolchain_version={}", self.toolchain_version),
            format!(
                "toolchain_executable_digest={}",
                self.toolchain_executable_digest
            ),
            format!("toolchain_selection={}", self.toolchain_selection),
            format!("goroot={}", self.goroot),
            format!("search_path={}", self.search_path),
            format!("host_target={}", self.host_target.label()),
            format!("environment_policy={}", self.environment_policy),
        ])
    }

    fn stamp(&self, executable_digest: &str) -> String {
        format!(
            "source_digest={}\ntoolchain_version={}\ntoolchain_executable_digest={}\ntoolchain_selection={}\ngoroot={}\nsearch_path={}\nhost_target={}\nenvironment_policy={}\nexecutable_digest={}\n",
            self.source_digest,
            self.toolchain_version,
            self.toolchain_executable_digest,
            self.toolchain_selection,
            self.goroot,
            self.search_path,
            self.host_target.label(),
            self.environment_policy,
            executable_digest
        )
    }
}

impl PreparedGoSemanticFrontend {
    pub(crate) fn prepare() -> Result<Self, GoSemanticProcessError> {
        let cache_root = default_frontend_cache_root()?;
        Self::prepare_with_cache_root(&cache_root)
    }

    fn prepare_with_cache_root(cache_root: &Path) -> Result<Self, GoSemanticProcessError> {
        ensure_private_cache_root(cache_root)?;
        let cache_root = canonical_private_cache_root(cache_root)?;
        let toolchain = prepare_go_toolchain(&cache_root, local_go_toolchain()?)?;
        match resolve_go_semantic_frontend_in(&cache_root)? {
            GoSemanticCommand::Binary(executable) => {
                prepare_binary_frontend(&cache_root, &executable, toolchain)
            }
            GoSemanticCommand::SourceDir(source_dir) => {
                let source = capture_source_snapshot(&source_dir)?;
                ensure_go_toolchain_supported(&toolchain.version)?;
                let source_dir = materialize_source_snapshot(&cache_root, &source)?;
                let provenance = FrontendBuildProvenance {
                    source_digest: source.digest.clone(),
                    toolchain_version: toolchain.version.clone(),
                    toolchain_executable_digest: toolchain.executable_digest.clone(),
                    toolchain_selection: normalized_path_label(&toolchain.canonical_selection),
                    goroot: normalized_path_label(&toolchain.goroot),
                    search_path: toolchain.search_path.to_string_lossy().into_owned(),
                    host_target: toolchain.host_target.clone(),
                    environment_policy: GO_ENVIRONMENT_POLICY,
                };
                let built =
                    ensure_frontend_binary(&cache_root, &source_dir, &provenance, &toolchain)?;
                let bytes = read_regular_file_no_follow(&built)?;
                let executable_digest = security_digest_bytes(&bytes);
                let executable = seal_executable(&cache_root, &bytes, &executable_digest)?;
                Ok(Self {
                    executable,
                    executable_digest,
                    source_digest: Some(source.digest),
                    toolchain: Box::new(toolchain),
                    environment_policy: GO_ENVIRONMENT_POLICY,
                })
            }
        }
    }

    pub(crate) fn command(&self, root: &Path) -> Command {
        let mut command = Command::new(&self.executable);
        command.current_dir(root);
        configure_go_environment(&mut command, &self.toolchain);
        command
    }

    pub(crate) fn identity_parts(&self) -> Vec<String> {
        vec![
            format!("executable_digest={}", self.executable_digest),
            format!(
                "source_digest={}",
                self.source_digest.as_deref().unwrap_or("prebuilt")
            ),
            format!("toolchain_version={}", self.toolchain.version),
            format!(
                "toolchain_executable_digest={}",
                self.toolchain.executable_digest
            ),
            format!(
                "toolchain_selection={}",
                normalized_path_label(&self.toolchain.canonical_selection)
            ),
            format!("goroot={}", normalized_path_label(&self.toolchain.goroot)),
            format!(
                "search_path={}",
                self.toolchain.search_path.to_string_lossy()
            ),
            format!("host_target={}", self.toolchain.host_target.label()),
            format!("environment_policy={}", self.environment_policy),
        ]
    }

    pub(crate) fn identity_digest(&self) -> String {
        let parts = self.identity_parts();
        security_digest_strings(&parts)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        executable_digest: &str,
        source_digest: Option<&str>,
        toolchain_version: Option<&str>,
    ) -> Self {
        let version = toolchain_version.unwrap_or("go-test").to_string();
        Self {
            executable: PathBuf::from("polint-go-frontend-test"),
            executable_digest: executable_digest.to_string(),
            source_digest: source_digest.map(str::to_string),
            toolchain: Box::new(PreparedGoToolchain {
                executable: PathBuf::from("go-test"),
                executable_digest: format!("go-executable-{version}"),
                canonical_selection: PathBuf::from("/test/bin/go"),
                version,
                host_target: GoHostTarget {
                    os: "test".to_string(),
                    arch: "test".to_string(),
                },
                goroot: PathBuf::from("/test/go"),
                search_path: OsString::from("/test/bin"),
                runtime_search_path: OsString::from("/sealed/toolchain:/test/bin"),
            }),
            environment_policy: GO_ENVIRONMENT_POLICY,
        }
    }
}

fn prepare_binary_frontend(
    cache_root: &Path,
    executable: &Path,
    toolchain: PreparedGoToolchain,
) -> Result<PreparedGoSemanticFrontend, GoSemanticProcessError> {
    let bytes = read_regular_file_no_follow(executable)?;
    let executable_digest = security_digest_bytes(&bytes);
    let executable = seal_executable(cache_root, &bytes, &executable_digest)?;
    Ok(PreparedGoSemanticFrontend {
        executable,
        executable_digest,
        source_digest: None,
        toolchain: Box::new(toolchain),
        environment_policy: GO_ENVIRONMENT_POLICY,
    })
}

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticToolPreparation {
    NotInvoked {
        reason: String,
    },
    SetupMissing {
        reason: String,
        process_error: Option<GoSemanticProcessError>,
    },
    Ready(PreparedGoSemanticFrontend),
}

impl GoSemanticToolPreparation {
    pub(crate) fn not_invoked(reason: impl Into<String>) -> Self {
        Self::NotInvoked {
            reason: reason.into(),
        }
    }

    pub(crate) fn setup_missing(reason: impl Into<String>) -> Self {
        Self::SetupMissing {
            reason: reason.into(),
            process_error: None,
        }
    }

    pub(crate) fn prepare() -> Self {
        match PreparedGoSemanticFrontend::prepare() {
            Ok(frontend) => Self::Ready(frontend),
            Err(error) => Self::SetupMissing {
                reason: error.stable_reason().to_string(),
                process_error: Some(error),
            },
        }
    }
}

pub(crate) fn resolve_go_semantic_frontend() -> Result<GoSemanticCommand, GoSemanticProcessError> {
    let cache_root = default_frontend_cache_root()?;
    ensure_private_cache_root(&cache_root)?;
    resolve_go_semantic_frontend_in(&canonical_private_cache_root(&cache_root)?)
}

fn resolve_go_semantic_frontend_in(
    cache_root: &Path,
) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    if let Ok(path) = std::env::var(GO_SEMANTIC_FRONTEND_ENV)
        && !path.trim().is_empty()
    {
        return command_for_path(PathBuf::from(path));
    }
    if let Some(path) = installed_frontend_binary()? {
        return Ok(GoSemanticCommand::Binary(path));
    }
    let embedded = embedded_source_snapshot();
    materialize_source_snapshot(cache_root, &embedded).map(GoSemanticCommand::SourceDir)
}

pub(crate) fn command_for_path(path: PathBuf) -> Result<GoSemanticCommand, GoSemanticProcessError> {
    if path.is_file() {
        return Ok(GoSemanticCommand::Binary(path));
    }
    if path.join("go.mod").is_file() {
        return Ok(GoSemanticCommand::SourceDir(path));
    }
    Err(GoSemanticProcessError::CommandFailed(format!(
        "{GO_SEMANTIC_FRONTEND_ENV} must point to a polint-go-frontend binary or source directory."
    )))
}

fn installed_frontend_binary() -> Result<Option<PathBuf>, GoSemanticProcessError> {
    let executable = std::env::current_exe().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to resolve current executable: {error}"
        ))
    })?;
    let Some(directory) = executable.parent() else {
        return Ok(None);
    };
    let candidate = directory.join(frontend_binary_name());
    Ok(candidate.is_file().then_some(candidate))
}

fn frontend_binary_name() -> &'static str {
    if cfg!(windows) {
        "polint-go-frontend.exe"
    } else {
        "polint-go-frontend"
    }
}

#[cfg(unix)]
fn default_frontend_cache_root() -> Result<PathBuf, GoSemanticProcessError> {
    let home = std::env::var_os("HOME").ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(
            "HOME is required for the private Go semantic frontend cache.".to_string(),
        )
    })?;
    let home = PathBuf::from(home).canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to resolve the user home for the Go semantic frontend cache: {error}"
        ))
    })?;
    Ok(home
        .join(".cache")
        .join("polint")
        .join("go-frontend")
        .join(GO_FRONTEND_CACHE_VERSION))
}

#[cfg(not(unix))]
fn default_frontend_cache_root() -> Result<PathBuf, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn ensure_private_cache_root(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let root = normalized_private_cache_root(root)?;
    let root = root.as_path();
    validate_private_cache_ancestors(root)?;
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache root `{}` must not be a symlink.",
                root.display()
            )));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache root `{}` is not a directory.",
                root.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(root).map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to create private Go semantic frontend cache `{}`: {error}",
                    root.display()
                ))
            })?;
        }
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend cache root `{}`: {error}",
                root.display()
            )));
        }
    }

    let metadata = fs::symlink_metadata(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend cache root `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is unsafe.",
            root.display()
        )));
    }
    if metadata.uid() != effective_user_id() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is not owned by the current user.",
            root.display()
        )));
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to restrict Go semantic frontend cache root `{}`: {error}",
            root.display()
        ))
    })?;
    let mode = fs::symlink_metadata(root)
        .map_err(|_| {
            GoSemanticProcessError::CommandFailed(
                "Go semantic frontend cache root became unavailable.".to_string(),
            )
        })?
        .mode();
    if mode & 0o077 != 0 {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend cache root `{}` is accessible by another user.",
            root.display()
        )));
    }
    validate_private_cache_ancestors(root)?;
    Ok(())
}

#[cfg(unix)]
fn canonical_private_cache_root(root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let normalized = normalized_private_cache_root(root)?;
    let canonical = normalized.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache `{}`: {error}",
            root.display()
        ))
    })?;
    validate_private_cache_ancestors(&canonical)?;
    Ok(canonical)
}

#[cfg(unix)]
fn normalized_private_cache_root(root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    use std::os::unix::fs::MetadataExt;

    if !root.is_absolute() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must be absolute.".to_string(),
        ));
    }
    if root.components().any(|component| {
        !matches!(
            component,
            std::path::Component::RootDir | std::path::Component::Normal(_)
        )
    }) {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must not contain relative path components."
                .to_string(),
        ));
    }

    let mut current = PathBuf::new();
    for component in root.components() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache ancestor `{}`: {error}",
                    current.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() && (current == root || metadata.uid() != 0) {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` must not be a symlink.",
                current.display()
            )));
        }
    }

    let mut existing = root;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    )));
                };
                missing.push(name.to_os_string());
                existing = existing.parent().ok_or_else(|| {
                    GoSemanticProcessError::CommandFailed(format!(
                        "failed to find an existing ancestor for private Go semantic frontend cache `{}`.",
                        root.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect private Go semantic frontend cache `{}`: {error}",
                    existing.display()
                )));
            }
        }
    }
    let mut normalized = existing.canonicalize().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize private Go semantic frontend cache ancestor `{}`: {error}",
            existing.display()
        ))
    })?;
    for component in missing.into_iter().rev() {
        normalized.push(component);
    }
    validate_private_cache_ancestors(&normalized)?;
    Ok(normalized)
}

#[cfg(unix)]
fn validate_private_cache_ancestors(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::MetadataExt;

    if !root.is_absolute() {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend cache root must be absolute.".to_string(),
        ));
    }
    let mut current = PathBuf::new();
    let mut missing_ancestor = false;
    for component in root.components() {
        current.push(component.as_os_str());
        if missing_ancestor {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_ancestor = true;
                continue;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache ancestor `{}`: {error}",
                    current.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` must not be a symlink.",
                current.display()
            )));
        }
        if !metadata.is_dir() {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` is not a directory.",
                current.display()
            )));
        }
        let mode = metadata.mode();
        let owner_can_replace =
            metadata.uid() != 0 && metadata.uid() != effective_user_id() && mode & 0o200 != 0;
        let shared_writable_without_sticky = mode & 0o022 != 0 && mode & 0o1000 == 0;
        if owner_can_replace || shared_writable_without_sticky {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend cache ancestor `{}` is mutable by another user.",
                current.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn effective_user_id() -> u32 {
    // SAFETY: `geteuid` takes no arguments and has no memory-safety preconditions.
    unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn ensure_private_cache_root(_root: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(not(unix))]
fn canonical_private_cache_root(_root: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn ensure_private_subdirectory(
    root: &Path,
    relative_path: &Path,
) -> Result<PathBuf, GoSemanticProcessError> {
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(GoSemanticProcessError::CommandFailed(
                "invalid Go semantic frontend cache path.".to_string(),
            ));
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` is unsafe.",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                create_private_directory(&current)?;
            }
            Err(error) => {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "failed to inspect Go semantic frontend cache directory `{}`: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(current)
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|_| {
                GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` became unavailable.",
                    path.display()
                ))
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(GoSemanticProcessError::CommandFailed(format!(
                    "Go semantic frontend cache directory `{}` is unsafe.",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to create Go semantic frontend cache directory `{}`: {error}",
            path.display()
        ))),
    }
}

#[cfg(not(unix))]
fn create_private_directory(_path: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

struct StagingDirectory {
    path: PathBuf,
    cleanup: bool,
}

impl StagingDirectory {
    fn create(parent: &Path, prefix: &str) -> Result<Self, GoSemanticProcessError> {
        cleanup_stale_staging_directories(parent, prefix, GO_FRONTEND_STALE_STAGING_AGE);
        let directory = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(parent)
            .map_err(|error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to allocate private Go semantic frontend staging directory: {error}"
                ))
            })?;
        Ok(Self {
            path: directory.keep(),
            cleanup: true,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn mark_published(&mut self) {
        self.cleanup = false;
    }
}

fn cleanup_stale_staging_directories(
    parent: &Path,
    prefix: &str,
    minimum_age: std::time::Duration,
) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.take(GO_FRONTEND_MAX_STALE_STAGING_SCAN).flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !name.starts_with(prefix) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if modified.elapsed().unwrap_or_default() < minimum_age {
            continue;
        }
        let _ = make_directory_tree_writable(&entry.path());
        let _ = fs::remove_dir_all(entry.path());
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.cleanup {
            return;
        }
        let _ = make_directory_tree_writable(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn embedded_frontend_hash() -> String {
    embedded_source_snapshot().digest
}

pub(crate) fn frontend_digest(
    frontend: &GoSemanticCommand,
) -> Result<String, GoSemanticProcessError> {
    match frontend {
        GoSemanticCommand::Binary(path) => {
            let bytes = read_regular_file_no_follow(path)?;
            Ok(security_digest_bytes(&bytes))
        }
        GoSemanticCommand::SourceDir(path) => Ok(capture_source_snapshot(path)?.digest),
    }
}

fn embedded_source_snapshot() -> FrontendSourceSnapshot {
    let mut files = EMBEDDED_GO_FRONTEND_FILES
        .iter()
        .map(|(relative_path, contents)| FrontendSourceFile {
            relative_path: PathBuf::from(relative_path),
            bytes: contents.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let digest = source_snapshot_digest(&files);
    FrontendSourceSnapshot { digest, files }
}

#[cfg(unix)]
fn capture_source_snapshot(path: &Path) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    let root = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to anchor Go semantic frontend source directory `{}`: {error}",
                path.display()
            ))
        })?;
    if !root
        .metadata()
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend source directory `{}`: {error}",
                path.display()
            ))
        })?
        .is_dir()
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source `{}` is not a directory.",
            path.display()
        )));
    }

    let mut frontier = vec![(OwnedFd::from(root), PathBuf::new(), 0_usize)];
    let mut visited_entries = 0_usize;
    let mut visited_directories = 1_usize;
    let mut captured = Vec::new();
    let mut total_bytes = 0_usize;

    while let Some((directory, relative_directory, depth)) = frontier.pop() {
        let mut names = directory_entry_names(&directory, &mut visited_entries)?;
        names.sort();
        for name in names.into_iter().rev() {
            let relative_path = relative_directory.join(&name);
            let name = CString::new(name.as_bytes()).map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "Go semantic frontend source contains an invalid path.".to_string(),
                )
            })?;
            let directory_flags =
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY;
            match open_frontend_at(directory.as_raw_fd(), &name, directory_flags) {
                Ok(child) => {
                    if skip_frontend_digest_dir(&relative_path) {
                        continue;
                    }
                    let child_depth = depth.saturating_add(1);
                    if child_depth > GO_FRONTEND_MAX_SOURCE_DEPTH {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source exceeds traversal depth {GO_FRONTEND_MAX_SOURCE_DEPTH}."
                        )));
                    }
                    visited_directories = visited_directories.checked_add(1).ok_or_else(|| {
                        GoSemanticProcessError::CommandFailed(
                            "Go semantic frontend source directory count overflowed.".to_string(),
                        )
                    })?;
                    if visited_directories > GO_FRONTEND_MAX_SOURCE_DIRECTORIES {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_DIRECTORIES} directories."
                        )));
                    }
                    frontier.push((child, relative_path, child_depth));
                    if frontier.len() > GO_FRONTEND_MAX_SOURCE_FRONTIER {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source traversal frontier exceeds {GO_FRONTEND_MAX_SOURCE_FRONTIER} directories."
                        )));
                    }
                }
                Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {}
                Err(error) if error.raw_os_error() == Some(libc::ENOTDIR) => {
                    if !is_frontend_digest_source(&relative_path) {
                        continue;
                    }
                    if captured.len() >= GO_FRONTEND_MAX_SOURCE_FILES {
                        return Err(GoSemanticProcessError::CommandFailed(format!(
                            "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_FILES} files."
                        )));
                    }
                    let file = open_frontend_at(
                        directory.as_raw_fd(),
                        &name,
                        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
                    )
                    .map_err(|error| {
                        GoSemanticProcessError::CommandFailed(format!(
                            "failed to open Go semantic frontend source `{}`: {error}",
                            relative_path.display()
                        ))
                    })?;
                    let remaining = GO_FRONTEND_MAX_SOURCE_BYTES.saturating_sub(total_bytes);
                    let bytes =
                        read_open_frontend_source(fs::File::from(file), &relative_path, remaining)?;
                    total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
                        GoSemanticProcessError::CommandFailed(
                            "Go semantic frontend source size overflowed.".to_string(),
                        )
                    })?;
                    captured.push(FrontendSourceFile {
                        relative_path,
                        bytes,
                    });
                }
                Err(error) => {
                    return Err(GoSemanticProcessError::CommandFailed(format!(
                        "failed to inspect Go semantic frontend source `{}`: {error}",
                        relative_path.display()
                    )));
                }
            }
        }
    }
    captured.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if !captured
        .iter()
        .any(|file| file.relative_path == Path::new("go.mod"))
    {
        return Err(GoSemanticProcessError::CommandFailed(
            "Go semantic frontend source snapshot has no go.mod.".to_string(),
        ));
    }
    let digest = source_snapshot_digest(&captured);
    Ok(FrontendSourceSnapshot {
        digest,
        files: captured,
    })
}

#[cfg(not(unix))]
fn capture_source_snapshot(_path: &Path) -> Result<FrontendSourceSnapshot, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "secure Go semantic frontend source traversal is unavailable on this platform.".to_string(),
    ))
}

fn source_snapshot_digest(files: &[FrontendSourceFile]) -> String {
    let mut parts = vec![GO_SEMANTIC_SCHEMA.to_string()];
    parts.extend(files.iter().map(|file| {
        format!(
            "{}:{}",
            file.relative_path.to_string_lossy().replace('\\', "/"),
            security_digest_bytes(&file.bytes)
        )
    }));
    security_digest_strings(&parts)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn directory_entry_names(
    directory: &std::os::fd::OwnedFd,
    visited_entries: &mut usize,
) -> Result<Vec<std::ffi::OsString>, GoSemanticProcessError> {
    use std::ffi::{CStr, OsString};
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStringExt;

    let duplicated = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicated < 0 {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to duplicate Go semantic frontend source directory handle: {}",
            std::io::Error::last_os_error()
        )));
    }
    let stream = unsafe { libc::fdopendir(duplicated) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe { libc::close(duplicated) };
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to enumerate Go semantic frontend source directory: {error}"
        )));
    }

    struct DirectoryStream(*mut libc::DIR);
    impl Drop for DirectoryStream {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }

    let stream = DirectoryStream(stream);
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        *visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
            GoSemanticProcessError::CommandFailed(
                "Go semantic frontend source entry count overflowed.".to_string(),
            )
        })?;
        if *visited_entries > GO_FRONTEND_MAX_SOURCE_ENTRIES {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "Go semantic frontend source contains more than {GO_FRONTEND_MAX_SOURCE_ENTRIES} entries."
            )));
        }
        names.push(OsString::from_vec(name.to_vec()));
    }
    Ok(names)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn open_frontend_at(
    directory: std::os::fd::RawFd,
    name: &std::ffi::CStr,
    flags: libc::c_int,
) -> std::io::Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;

    let descriptor = unsafe { libc::openat(directory, name.as_ptr(), flags) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(descriptor) })
}

#[cfg(unix)]
fn read_open_frontend_source(
    mut file: fs::File,
    relative_path: &Path,
    remaining_bytes: usize,
) -> Result<Vec<u8>, GoSemanticProcessError> {
    use std::io::Read;

    let metadata = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend source `{}`: {error}",
            relative_path.display()
        ))
    })?;
    let file_len = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if !metadata.is_file() || file_len > remaining_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source exceeds {GO_FRONTEND_MAX_SOURCE_BYTES} bytes."
        )));
    }
    let mut bytes = Vec::with_capacity(file_len);
    std::io::Read::by_ref(&mut file)
        .take(
            u64::try_from(remaining_bytes)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to read Go semantic frontend source `{}`: {error}",
                relative_path.display()
            ))
        })?;
    if bytes.len() > remaining_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend source exceeds {GO_FRONTEND_MAX_SOURCE_BYTES} bytes."
        )));
    }
    Ok(bytes)
}

fn skip_frontend_digest_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".hg" | "target" | "node_modules")
    )
}

fn is_frontend_digest_source(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("go.mod" | "go.sum")
    ) || path.extension().and_then(|extension| extension.to_str()) == Some("go")
}

fn materialize_source_snapshot(
    cache_root: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<PathBuf, GoSemanticProcessError> {
    let sources_root = ensure_private_subdirectory(cache_root, Path::new("sources"))?;
    let destination = sources_root.join(&snapshot.digest);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "cached Go semantic frontend source `{}` is unsafe.",
                destination.display()
            )));
        }
        Ok(_) => {
            if source_snapshot_matches(&destination, snapshot)? {
                return Ok(destination);
            }
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "cached Go semantic frontend source `{}` failed content verification.",
                destination.display()
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect cached Go semantic frontend source `{}`: {error}",
                destination.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create(&sources_root, ".source-")?;
    for file in &snapshot.files {
        let path = staging.path().join(&file.relative_path);
        if let Some(parent) = file.relative_path.parent()
            && !parent.as_os_str().is_empty()
        {
            ensure_private_subdirectory(staging.path(), parent)?;
        }
        write_new_private_file(&path, &file.bytes, false)?;
    }
    make_source_snapshot_read_only(staging.path(), snapshot)?;

    match fs::rename(staging.path(), &destination) {
        Ok(()) => {
            staging.mark_published();
            Ok(destination)
        }
        Err(_) if destination.is_dir() => {
            if source_snapshot_matches(&destination, snapshot)? {
                Ok(destination)
            } else {
                Err(GoSemanticProcessError::CommandFailed(format!(
                    "concurrently published Go semantic frontend source `{}` failed verification.",
                    destination.display()
                )))
            }
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish Go semantic frontend source `{}`: {error}",
            destination.display()
        ))),
    }
}

fn source_snapshot_matches(
    directory: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<bool, GoSemanticProcessError> {
    let expected_files = snapshot
        .files
        .iter()
        .map(|file| (file.relative_path.clone(), file.bytes.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let expected_directories = snapshot
        .files
        .iter()
        .flat_map(|file| file.relative_path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>();
    let mut remaining = expected_files;
    if !verify_source_directory(directory, directory, &expected_directories, &mut remaining)? {
        return Ok(false);
    }
    Ok(remaining.is_empty())
}

fn verify_source_directory(
    root: &Path,
    directory: &Path,
    expected_directories: &BTreeSet<PathBuf>,
    remaining: &mut BTreeMap<PathBuf, &[u8]>,
) -> Result<bool, GoSemanticProcessError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to audit Go semantic frontend source directory `{}`: {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to audit Go semantic frontend source entry: {error}"
            ))
        })?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| {
            GoSemanticProcessError::CommandFailed(
                "cached Go semantic frontend source escaped its root.".to_string(),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to audit cached Go semantic frontend source `{}`: {error}",
                path.display()
            ))
        })?;
        if file_type.is_symlink() {
            return Ok(false);
        }
        if file_type.is_dir() {
            if !expected_directories.contains(relative)
                || !verify_source_directory(root, &path, expected_directories, remaining)?
            {
                return Ok(false);
            }
        } else if file_type.is_file() {
            let Some(expected) = remaining.remove(relative) else {
                return Ok(false);
            };
            if read_regular_file_no_follow(&path)? != expected {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}

fn security_digest_strings(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let bytes = part.as_bytes();
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

fn security_digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn normalized_path_label(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn ensure_frontend_binary(
    cache_root: &Path,
    source_dir: &Path,
    provenance: &FrontendBuildProvenance,
    toolchain: &PreparedGoToolchain,
) -> Result<PathBuf, GoSemanticProcessError> {
    ensure_go_toolchain_supported(&provenance.toolchain_version)?;
    let builds_root = ensure_private_subdirectory(cache_root, Path::new("builds"))?;
    let destination = builds_root.join(provenance.cache_key());
    if destination.exists() {
        return verify_cached_build(&destination, provenance);
    }

    let mut staging = StagingDirectory::create(&builds_root, ".build-")?;
    let binary = staging.path().join(frontend_binary_name());
    let mut command = Command::new(&toolchain.executable);
    configure_go_environment(&mut command, toolchain);
    let output = command
        .arg("build")
        .arg("-trimpath")
        .arg("-o")
        .arg(&binary)
        .arg(".")
        .current_dir(source_dir)
        .env("GOWORK", "off")
        .output()
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to start reproducible Go semantic frontend build: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend build failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let executable_digest = security_digest_bytes(&read_regular_file_no_follow(&binary)?);
    write_new_private_file(
        &staging.path().join("provenance"),
        provenance.stamp(&executable_digest).as_bytes(),
        false,
    )?;
    make_file_executable_read_only(&binary)?;

    match fs::rename(staging.path(), &destination) {
        Ok(()) => {
            staging.mark_published();
            Ok(destination.join(frontend_binary_name()))
        }
        Err(_) if destination.is_dir() => verify_cached_build(&destination, provenance),
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish verified Go semantic frontend build `{}`: {error}",
            destination.display()
        ))),
    }
}

fn verify_cached_build(
    directory: &Path,
    provenance: &FrontendBuildProvenance,
) -> Result<PathBuf, GoSemanticProcessError> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect cached Go semantic frontend build `{}`: {error}",
            directory.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "cached Go semantic frontend build `{}` is unsafe.",
            directory.display()
        )));
    }
    let binary = directory.join(frontend_binary_name());
    let executable_digest = security_digest_bytes(&read_regular_file_no_follow(&binary)?);
    let stamp = read_regular_file_no_follow(&directory.join("provenance"))?;
    if stamp != provenance.stamp(&executable_digest).as_bytes() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "cached Go semantic frontend build `{}` failed provenance verification.",
            directory.display()
        )));
    }
    Ok(binary)
}

fn seal_executable(
    cache_root: &Path,
    bytes: &[u8],
    executable_digest: &str,
) -> Result<PathBuf, GoSemanticProcessError> {
    let execution_root = ensure_private_subdirectory(cache_root, Path::new("executables"))?;
    let directory = execution_root.join(executable_digest);
    match fs::symlink_metadata(&directory) {
        Ok(_) => return verify_sealed_executable(&directory, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect sealed Go semantic frontend directory `{}`: {error}",
                directory.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create(&execution_root, ".seal-")?;
    write_new_private_file(&staging.path().join(frontend_binary_name()), bytes, true)?;
    seal_execution_directory(staging.path())?;
    match fs::rename(staging.path(), &directory) {
        Ok(()) => {
            staging.mark_published();
            verify_sealed_executable(&directory, bytes)
        }
        Err(_) if fs::symlink_metadata(&directory).is_ok() => {
            verify_sealed_executable(&directory, bytes)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish sealed Go semantic frontend `{}`: {error}",
            directory.display()
        ))),
    }
}

fn verify_sealed_executable(
    directory: &Path,
    expected_bytes: &[u8],
) -> Result<PathBuf, GoSemanticProcessError> {
    let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go semantic frontend directory `{}`: {error}",
            directory.display()
        ))
    })?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend directory `{}` is unsafe.",
            directory.display()
        )));
    }
    let executable = directory.join(frontend_binary_name());
    if read_regular_file_no_follow(&executable)? != expected_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend `{}` failed content verification.",
            executable.display()
        )));
    }
    verify_sealed_permissions(directory, &directory_metadata, &executable)?;
    Ok(executable)
}

fn prepare_go_toolchain(
    cache_root: &Path,
    toolchain: GoToolchain,
) -> Result<PreparedGoToolchain, GoSemanticProcessError> {
    let bytes = read_regular_file_no_follow(&toolchain.executable)?;
    let digest = security_digest_bytes(&bytes);
    if digest != toolchain.executable_digest {
        return Err(GoSemanticProcessError::CommandFailed(
            "selected Go executable changed during preparation.".to_string(),
        ));
    }
    let executable = seal_go_toolchain_executable(cache_root, &bytes, &digest)?;
    let runtime_search_path = prepend_toolchain_path(&executable, &toolchain.search_path)?;
    let mut prepared = PreparedGoToolchain {
        executable,
        executable_digest: digest,
        canonical_selection: toolchain.canonical_selection,
        version: toolchain.version,
        host_target: toolchain.host_target,
        goroot: toolchain.goroot,
        search_path: toolchain.search_path,
        runtime_search_path,
    };
    let (version, host_target) = probe_prepared_go_toolchain(&prepared)?;
    if host_target != GoHostTarget::current_process()? {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go executable targets {}, but polint is running on {}.",
            host_target.label(),
            GoHostTarget::current_process()?.label()
        )));
    }
    prepared.version = version;
    prepared.host_target = host_target;
    Ok(prepared)
}

fn probe_prepared_go_toolchain(
    toolchain: &PreparedGoToolchain,
) -> Result<(String, GoHostTarget), GoSemanticProcessError> {
    let mut command = Command::new(&toolchain.executable);
    configure_go_environment(&mut command, toolchain);
    let output = command.arg("version").output().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to run sealed `go version` for Go semantic frontend: {error}"
        ))
    })?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed `go version` exited with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_go_toolchain(stdout.as_ref()).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to parse sealed Go toolchain version and host target from `{}`",
            stdout.trim()
        ))
    })
}

fn seal_go_toolchain_executable(
    cache_root: &Path,
    bytes: &[u8],
    executable_digest: &str,
) -> Result<PathBuf, GoSemanticProcessError> {
    let toolchains_root = ensure_private_subdirectory(cache_root, Path::new("toolchains"))?;
    let directory = toolchains_root.join(executable_digest);
    match fs::symlink_metadata(&directory) {
        Ok(_) => return verify_sealed_go_toolchain(&directory, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect sealed Go toolchain directory `{}`: {error}",
                directory.display()
            )));
        }
    }

    let mut staging = StagingDirectory::create(&toolchains_root, ".toolchain-")?;
    write_new_private_file(
        &staging.path().join(go_toolchain_binary_name()),
        bytes,
        true,
    )?;
    seal_execution_directory(staging.path())?;
    match fs::rename(staging.path(), &directory) {
        Ok(()) => {
            staging.mark_published();
            verify_sealed_go_toolchain(&directory, bytes)
        }
        Err(_) if fs::symlink_metadata(&directory).is_ok() => {
            verify_sealed_go_toolchain(&directory, bytes)
        }
        Err(error) => Err(GoSemanticProcessError::CommandFailed(format!(
            "failed to publish sealed Go toolchain `{}`: {error}",
            directory.display()
        ))),
    }
}

fn verify_sealed_go_toolchain(
    directory: &Path,
    expected_bytes: &[u8],
) -> Result<PathBuf, GoSemanticProcessError> {
    let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go toolchain directory `{}`: {error}",
            directory.display()
        ))
    })?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go toolchain directory `{}` is unsafe.",
            directory.display()
        )));
    }
    let executable = directory.join(go_toolchain_binary_name());
    if read_regular_file_no_follow(&executable)? != expected_bytes {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go toolchain `{}` failed content verification.",
            executable.display()
        )));
    }
    verify_sealed_permissions(directory, &directory_metadata, &executable)?;
    Ok(executable)
}

#[cfg(windows)]
fn go_toolchain_binary_name() -> &'static str {
    "go.exe"
}

#[cfg(not(windows))]
fn go_toolchain_binary_name() -> &'static str {
    "go"
}

#[cfg(unix)]
fn seal_execution_directory(directory: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(directory, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to make sealed Go semantic frontend directory immutable `{}`: {error}",
            directory.display()
        ))
    })
}

#[cfg(not(unix))]
fn seal_execution_directory(_directory: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn verify_sealed_permissions(
    _directory: &Path,
    directory_metadata: &fs::Metadata,
    executable: &Path,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let executable_metadata = fs::symlink_metadata(executable).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect sealed Go semantic frontend `{}`: {error}",
            executable.display()
        ))
    })?;
    let owned = directory_metadata.uid() == effective_user_id()
        && executable_metadata.uid() == effective_user_id();
    let modes_are_sealed = directory_metadata.permissions().mode() & 0o777 == 0o500
        && executable_metadata.permissions().mode() & 0o777 == 0o500;
    if !owned || !modes_are_sealed || !executable_metadata.is_file() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "sealed Go semantic frontend `{}` failed ownership or permission verification.",
            executable.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_sealed_permissions(
    _directory: &Path,
    _directory_metadata: &fs::Metadata,
    _executable: &Path,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn configure_go_environment(command: &mut Command, toolchain: &PreparedGoToolchain) {
    for variable in GO_ENVIRONMENT_MODIFIERS {
        command.env_remove(variable);
    }
    command
        .env("GOOS", &toolchain.host_target.os)
        .env("GOARCH", &toolchain.host_target.arch)
        .env("CGO_ENABLED", "0")
        .env("GOENV", "off")
        .env("GOFLAGS", "")
        .env("GO111MODULE", "on")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .env("GOROOT", &toolchain.goroot)
        .env("PATH", &toolchain.runtime_search_path);
}

fn prepend_toolchain_path(
    executable: &Path,
    search_path: &OsString,
) -> Result<OsString, GoSemanticProcessError> {
    let mut paths = executable
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .chain(std::env::split_paths(search_path))
        .collect::<Vec<_>>();
    paths.dedup();
    std::env::join_paths(paths).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to construct sealed Go toolchain search path: {error}"
        ))
    })
}

#[cfg(unix)]
fn write_new_private_file(
    path: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::OpenOptionsExt;

    let mode = if executable { 0o500 } else { 0o400 };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to create private Go semantic frontend file `{}`: {error}",
                path.display()
            ))
        })?;
    file.write_all(bytes).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to write private Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to sync private Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn write_new_private_file(
    _path: &Path,
    _bytes: &[u8],
    _executable: bool,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn read_regular_file_no_follow(path: &Path) -> Result<Vec<u8>, GoSemanticProcessError> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to open Go semantic frontend file `{}` without following symlinks: {error}",
                path.display()
            ))
        })?;
    let metadata = file.metadata().map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file()
        || metadata.len() > u64::try_from(GO_FRONTEND_MAX_EXECUTABLE_BYTES).unwrap_or(u64::MAX)
    {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend file `{}` is not a bounded regular file.",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or_default());
    file.take(
        u64::try_from(GO_FRONTEND_MAX_EXECUTABLE_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
    )
    .read_to_end(&mut bytes)
    .map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to read Go semantic frontend file `{}`: {error}",
            path.display()
        ))
    })?;
    if bytes.len() > GO_FRONTEND_MAX_EXECUTABLE_BYTES {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "Go semantic frontend file `{}` exceeds the size limit.",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_regular_file_no_follow(_path: &Path) -> Result<Vec<u8>, GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_file_executable_read_only(path: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend executable `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn make_file_executable_read_only(_path: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_source_snapshot_read_only(
    root: &Path,
    snapshot: &FrontendSourceSnapshot,
) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    for file in &snapshot.files {
        fs::set_permissions(
            root.join(&file.relative_path),
            fs::Permissions::from_mode(0o400),
        )
        .map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to seal Go semantic frontend source `{}`: {error}",
                file.relative_path.display()
            ))
        })?;
    }
    let mut directories = snapshot
        .files
        .iter()
        .flat_map(|file| file.relative_path.ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        fs::set_permissions(root.join(directory), fs::Permissions::from_mode(0o500)).map_err(
            |error| {
                GoSemanticProcessError::CommandFailed(format!(
                    "failed to seal Go semantic frontend source directory: {error}"
                ))
            },
        )?;
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o500)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to seal Go semantic frontend source root `{}`: {error}",
            root.display()
        ))
    })
}

#[cfg(not(unix))]
fn make_source_snapshot_read_only(
    _root: &Path,
    _snapshot: &FrontendSourceSnapshot,
) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

#[cfg(unix)]
fn make_directory_tree_writable(root: &Path) -> Result<(), GoSemanticProcessError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to reopen Go semantic frontend staging directory `{}`: {error}",
            root.display()
        ))
    })?;
    for entry in fs::read_dir(root).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to inspect Go semantic frontend staging directory `{}`: {error}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to inspect Go semantic frontend staging entry: {error}"
            ))
        })?;
        if entry
            .file_type()
            .map_err(|_| {
                GoSemanticProcessError::CommandFailed(
                    "failed to inspect Go semantic frontend staging entry type.".to_string(),
                )
            })?
            .is_dir()
        {
            make_directory_tree_writable(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_directory_tree_writable(_root: &Path) -> Result<(), GoSemanticProcessError> {
    Err(GoSemanticProcessError::CommandUnavailable(
        "private Go semantic frontend execution is unavailable on this platform.".to_string(),
    ))
}

fn ensure_go_toolchain_supported(version: &str) -> Result<(), GoSemanticProcessError> {
    if !go_version_at_least(version, 1, 25) {
        return Err(GoSemanticProcessError::VersionUnsupported(format!(
            "polint-go-frontend source mode requires Go 1.25 or newer on PATH; found {version}"
        )));
    }
    Ok(())
}

pub(crate) fn local_go_toolchain_version() -> Result<String, GoSemanticProcessError> {
    local_go_toolchain().map(|toolchain| toolchain.version)
}

fn local_go_toolchain() -> Result<GoToolchain, GoSemanticProcessError> {
    let search_path = std::env::var_os("PATH").ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(
            "PATH is unavailable while resolving the Go semantic toolchain.".to_string(),
        )
    })?;
    local_go_toolchain_in(search_path)
}

fn local_go_toolchain_in(search_path: OsString) -> Result<GoToolchain, GoSemanticProcessError> {
    let canonical_selection = resolve_go_executable(&search_path)?;
    let executable_bytes = read_regular_file_no_follow(&canonical_selection)?;
    let executable_digest = security_digest_bytes(&executable_bytes);
    let target = GoHostTarget::current_process()?;

    let mut version_command = Command::new(&canonical_selection);
    configure_go_probe_environment(&mut version_command, &target, &search_path);
    let output = version_command.arg("version").output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GoSemanticProcessError::CommandUnavailable(
                "go executable was not found for the Go semantic frontend.".to_string(),
            )
        } else {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to run `go version` for Go semantic frontend: {error}"
            ))
        }
    })?;
    if !output.status.success() {
        return Err(GoSemanticProcessError::CommandFailed(format!(
            "`go version` exited with status {}",
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let (version, host_target) = parse_go_toolchain(stdout.as_ref()).ok_or_else(|| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to parse Go toolchain version and host target from `{}`",
            stdout.trim()
        ))
    })?;

    let goroot = infer_go_root(&canonical_selection)?;

    Ok(GoToolchain {
        executable: canonical_selection.clone(),
        executable_digest,
        canonical_selection,
        version,
        host_target,
        goroot,
        search_path,
    })
}

fn infer_go_root(executable: &Path) -> Result<PathBuf, GoSemanticProcessError> {
    let goroot = executable.parent().and_then(Path::parent).ok_or_else(|| {
        GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go executable `{}` is not located under GOROOT/bin.",
            executable.display()
        ))
    })?;
    let goroot = fs::canonicalize(goroot).map_err(|error| {
        GoSemanticProcessError::CommandFailed(format!(
            "failed to canonicalize selected Go toolchain root `{}`: {error}",
            goroot.display()
        ))
    })?;
    if !goroot.is_dir() {
        return Err(GoSemanticProcessError::CommandUnavailable(format!(
            "selected Go toolchain root `{}` is not a directory.",
            goroot.display()
        )));
    }
    Ok(goroot)
}

fn resolve_go_executable(search_path: &OsString) -> Result<PathBuf, GoSemanticProcessError> {
    for directory in std::env::split_paths(search_path) {
        let candidate = directory.join(go_toolchain_binary_name());
        let Ok(metadata) = fs::symlink_metadata(&candidate) else {
            continue;
        };
        if metadata.is_dir() {
            continue;
        }
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            GoSemanticProcessError::CommandFailed(format!(
                "failed to canonicalize selected Go executable `{}`: {error}",
                candidate.display()
            ))
        })?;
        if canonical.is_file() {
            return Ok(canonical);
        }
    }
    Err(GoSemanticProcessError::CommandUnavailable(
        "go executable was not found for the Go semantic frontend.".to_string(),
    ))
}

fn configure_go_probe_environment(
    command: &mut Command,
    target: &GoHostTarget,
    search_path: &OsString,
) {
    for variable in GO_ENVIRONMENT_MODIFIERS {
        command.env_remove(variable);
    }
    command
        .env("GOOS", &target.os)
        .env("GOARCH", &target.arch)
        .env("CGO_ENABLED", "0")
        .env("GOENV", "off")
        .env("GOFLAGS", "")
        .env("GO111MODULE", "on")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .env("PATH", search_path);
}

fn parse_go_toolchain(output: &str) -> Option<(String, GoHostTarget)> {
    let mut parts = output.split_whitespace();
    let _go = parts.next()?;
    let _version_word = parts.next()?;
    let version = parts.next()?.to_string();
    let host_target = parts.next().and_then(GoHostTarget::parse_label)?;
    Some((version, host_target))
}

fn parse_go_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|part| part.starts_with("go") && part.len() > 2)
        .map(str::to_string)
}

fn go_version_at_least(version: &str, min_major: u32, min_minor: u32) -> bool {
    let Some((major, minor)) = parse_go_version_numbers(version) else {
        return false;
    };
    major > min_major || (major == min_major && minor >= min_minor)
}

fn parse_go_version_numbers(version: &str) -> Option<(u32, u32)> {
    let version = version.trim().trim_start_matches("go");
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn test_cache_root(temp: &tempfile::TempDir) -> PathBuf {
        temp.path().join("cache")
    }

    fn prepared_toolchain_for_test(version: &str) -> PreparedGoToolchain {
        PreparedGoToolchain {
            executable: PathBuf::from("/sealed/toolchain/go"),
            executable_digest: format!("digest-{version}"),
            canonical_selection: PathBuf::from("/selected/bin/go"),
            version: version.to_string(),
            host_target: GoHostTarget {
                os: "linux".to_string(),
                arch: "amd64".to_string(),
            },
            goroot: PathBuf::from("/selected/go"),
            search_path: OsString::from("/selected/bin:/usr/bin:/bin"),
            runtime_search_path: OsString::from("/sealed/toolchain:/selected/bin:/usr/bin:/bin"),
        }
    }

    fn build_provenance_for_test(version: &str) -> FrontendBuildProvenance {
        let toolchain = prepared_toolchain_for_test(version);
        FrontendBuildProvenance {
            source_digest: "source".to_string(),
            toolchain_version: toolchain.version,
            toolchain_executable_digest: toolchain.executable_digest,
            toolchain_selection: normalized_path_label(&toolchain.canonical_selection),
            goroot: normalized_path_label(&toolchain.goroot),
            search_path: toolchain.search_path.to_string_lossy().into_owned(),
            host_target: toolchain.host_target,
            environment_policy: GO_ENVIRONMENT_POLICY,
        }
    }

    #[test]
    fn command_for_path_accepts_source_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("go.mod"), "module test\n").expect("write go.mod");
        let command = command_for_path(temp.path().to_path_buf()).expect("source dir accepted");
        assert!(matches!(command, GoSemanticCommand::SourceDir(_)));
    }

    #[test]
    fn embedded_frontend_hash_includes_schema() {
        assert_eq!(embedded_frontend_hash().len(), 64);
    }

    #[test]
    fn security_content_digest_is_full_sha256() {
        assert_eq!(
            security_digest_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn embedded_go_frontend_sources_match_workspace_sources() {
        let workspace_frontend =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("go-sidecar/polint-go-frontend");
        for (relative_path, embedded) in EMBEDDED_GO_FRONTEND_FILES {
            let workspace = fs::read_to_string(workspace_frontend.join(relative_path))
                .unwrap_or_else(|error| panic!("read workspace frontend {relative_path}: {error}"));
            assert_eq!(
                workspace, *embedded,
                "embedded Go semantic frontend drifted at {relative_path}"
            );
        }
    }

    #[test]
    fn source_dir_digest_changes_when_go_source_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let first = frontend_digest(&GoSemanticCommand::SourceDir(temp.path().to_path_buf()))
            .expect("digest source dir");

        fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() { println(\"changed\") }\n",
        )
        .expect("rewrite main.go");
        let second = frontend_digest(&GoSemanticCommand::SourceDir(temp.path().to_path_buf()))
            .expect("digest source dir again");

        assert_ne!(first, second);
    }

    #[test]
    fn go_toolchain_version_parser_accepts_go_version_output() {
        assert_eq!(
            parse_go_version("go version go1.26.2 darwin/arm64"),
            Some("go1.26.2".to_string())
        );
        assert!(go_version_at_least("go1.25.0", 1, 25));
        assert!(!go_version_at_least("go1.24.9", 1, 25));
        assert_eq!(
            parse_go_toolchain("go version go1.26.2 darwin/arm64"),
            Some((
                "go1.26.2".to_string(),
                GoHostTarget {
                    os: "darwin".to_string(),
                    arch: "arm64".to_string(),
                }
            ))
        );
    }

    #[test]
    fn prepared_frontend_identity_tracks_executable_source_and_toolchain() {
        let baseline = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-a"),
            Some("go1.25.0"),
        );
        let executable_changed = PreparedGoSemanticFrontend::for_test(
            "executable-b",
            Some("source-a"),
            Some("go1.25.0"),
        );
        let source_changed = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-b"),
            Some("go1.25.0"),
        );
        let toolchain_changed = PreparedGoSemanticFrontend::for_test(
            "executable-a",
            Some("source-a"),
            Some("go1.26.0"),
        );

        assert_ne!(
            baseline.identity_digest(),
            executable_changed.identity_digest()
        );
        assert_ne!(baseline.identity_digest(), source_changed.identity_digest());
        assert_ne!(
            baseline.identity_digest(),
            toolchain_changed.identity_digest()
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_symlink() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let cache = test_cache_root(&temp);
        std::os::unix::fs::symlink(outside.path(), &cache).expect("symlink cache root");

        let error = ensure_private_cache_root(&cache).expect_err("symlink root must fail");

        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_symlink_ancestor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        let ancestor = temp.path().join("cache-link");
        std::os::unix::fs::symlink(outside.path(), &ancestor).expect("symlink cache ancestor");
        let cache = ancestor.join("cache");

        let error = ensure_private_cache_root(&cache).expect_err("symlink ancestor must fail");

        assert!(error.to_string().contains("ancestor"));
        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn private_cache_root_rejects_parent_components() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = temp.path().join("missing").join("..").join("cache");

        let error = ensure_private_cache_root(&cache).expect_err("parent component must fail");

        assert!(error.to_string().contains("relative path components"));
    }

    #[cfg(unix)]
    #[test]
    fn sealed_executable_rejects_hostile_preseed_with_wrong_bytes() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let expected = b"#!/bin/sh\nprintf 'expected\\n'\n";
        let digest = security_digest_bytes(expected);
        let directory =
            ensure_private_subdirectory(&cache, &Path::new("executables").join(&digest))
                .expect("execution directory");
        let executable = directory.join(frontend_binary_name());
        fs::write(&executable, b"#!/bin/sh\nprintf 'hostile\\n'\n").expect("preseed binary");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
            .expect("executable mode");

        let error = seal_executable(&cache, expected, &digest).expect_err("preseed must fail");

        assert!(error.to_string().contains("failed content verification"));
    }

    #[cfg(unix)]
    #[test]
    fn prepared_binary_executes_sealed_bytes_after_source_path_is_replaced() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let source = temp.path().join("frontend");
        fs::write(&source, b"#!/bin/sh\nprintf 'A\\n'\n").expect("write frontend A");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).expect("frontend mode");
        let prepared =
            prepare_binary_frontend(&cache, &source, prepared_toolchain_for_test("go1.25.0"))
                .expect("prepare frontend A");
        fs::write(&source, b"#!/bin/sh\nprintf 'B\\n'\n").expect("replace source with B");

        let output = prepared
            .command(temp.path())
            .output()
            .expect("run sealed frontend");

        assert_eq!(String::from_utf8_lossy(&output.stdout), "A\n");
        fs::set_permissions(
            prepared.executable.parent().expect("sealed directory"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("reopen sealed directory for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn sealed_execution_target_rejects_direct_mutation_and_replacement() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let source = temp.path().join("frontend");
        fs::write(&source, b"#!/bin/sh\nprintf 'sealed\\n'\n").expect("write frontend");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).expect("frontend mode");
        let prepared =
            prepare_binary_frontend(&cache, &source, prepared_toolchain_for_test("go1.25.0"))
                .expect("prepare frontend");
        let sealed_directory = prepared.executable.parent().expect("sealed directory");
        let source_bytes = read_regular_file_no_follow(&source).expect("read source bytes");
        let reused = seal_executable(&cache, &source_bytes, &security_digest_bytes(&source_bytes))
            .expect("reuse verified sealed frontend");
        let replacement = temp.path().join("replacement");
        fs::write(&replacement, b"#!/bin/sh\nprintf 'replacement\\n'\n")
            .expect("write replacement");

        let directory_mode = fs::metadata(sealed_directory)
            .expect("sealed directory metadata")
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o500);
        assert_eq!(reused, prepared.executable);
        if effective_user_id() != 0 {
            assert!(
                fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&prepared.executable)
                    .is_err()
            );
            assert!(fs::rename(&replacement, &prepared.executable).is_err());
        }
        fs::set_permissions(sealed_directory, fs::Permissions::from_mode(0o700))
            .expect("reopen sealed directory for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn materialized_source_rejects_completion_marker_preseed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let snapshot = embedded_source_snapshot();
        let directory =
            ensure_private_subdirectory(&cache, &Path::new("sources").join(&snapshot.digest))
                .expect("preseed source directory");
        fs::write(directory.join(".complete"), "").expect("preseed marker");

        let error = materialize_source_snapshot(&cache, &snapshot)
            .expect_err("completion marker alone must not be trusted");

        assert!(error.to_string().contains("failed content verification"));
    }

    #[cfg(unix)]
    #[test]
    fn cached_source_build_is_rejected_before_lookup_for_old_toolchain() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let provenance = build_provenance_for_test("go1.24.9");
        let toolchain = prepared_toolchain_for_test("go1.24.9");
        ensure_private_subdirectory(&cache, &Path::new("builds").join(provenance.cache_key()))
            .expect("preseed old build cache");

        let error = ensure_frontend_binary(&cache, temp.path(), &provenance, &toolchain)
            .expect_err("old toolchain cache must fail");

        assert!(matches!(
            error,
            GoSemanticProcessError::VersionUnsupported(_)
        ));
    }

    #[test]
    fn source_build_cache_key_binds_source_toolchain_target_and_environment() {
        let mut baseline = build_provenance_for_test("go1.25.0");
        baseline.source_digest = "source-a".to_string();
        let mut source_changed = baseline.clone();
        source_changed.source_digest = "source-b".to_string();
        let mut toolchain_changed = baseline.clone();
        toolchain_changed.toolchain_version = "go1.26.0".to_string();
        let mut executable_changed = baseline.clone();
        executable_changed.toolchain_executable_digest = "other-executable".to_string();
        let mut selection_changed = baseline.clone();
        selection_changed.toolchain_selection = "/other/bin/go".to_string();
        let mut goroot_changed = baseline.clone();
        goroot_changed.goroot = "/other/go".to_string();
        let mut search_path_changed = baseline.clone();
        search_path_changed.search_path = "/other/bin".to_string();
        let mut target_changed = baseline.clone();
        target_changed.host_target.arch = "arm64".to_string();
        let mut environment_changed = baseline.clone();
        environment_changed.environment_policy = "different-policy";

        let keys = [
            baseline.cache_key(),
            source_changed.cache_key(),
            toolchain_changed.cache_key(),
            executable_changed.cache_key(),
            selection_changed.cache_key(),
            goroot_changed.cache_key(),
            search_path_changed.cache_key(),
            target_changed.cache_key(),
            environment_changed.cache_key(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        assert_eq!(keys.len(), 9);
        assert!(keys.iter().all(|key| key.len() == 64));
    }

    #[test]
    fn command_environment_overrides_semantic_go_modifiers() {
        let mut command = Command::new("unused");
        command
            .env("GOOS", "hostile-os")
            .env("GOARCH", "hostile-arch")
            .env("GOFLAGS", "-tags=hostile")
            .env("GOAMD64", "v4")
            .env("CGO_ENABLED", "1");
        let toolchain = prepared_toolchain_for_test("go1.25.0");

        configure_go_environment(&mut command, &toolchain);

        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(environment.get("GOOS"), Some(&Some("linux".to_string())));
        assert_eq!(environment.get("GOARCH"), Some(&Some("amd64".to_string())));
        assert_eq!(environment.get("GOFLAGS"), Some(&Some(String::new())));
        assert_eq!(environment.get("CGO_ENABLED"), Some(&Some("0".to_string())));
        assert_eq!(environment.get("GOAMD64"), Some(&None));
        assert_eq!(
            environment.get("GOROOT"),
            Some(&Some("/selected/go".to_string()))
        );
        assert!(
            environment
                .get("PATH")
                .and_then(Option::as_ref)
                .is_some_and(|path| path.starts_with("/sealed/toolchain"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepared_frontend_pins_selected_go_toolchain_after_launcher_replacement() {
        use std::os::unix::fs::PermissionsExt;

        fn write_launcher(path: &Path, label: &str, target: &str, goroot: &Path) {
            let script = format!(
                "#!/bin/sh\nif [ \"$1\" = version ]; then\n  printf 'go version go1.25.0 {target} {label}\\n'\nelif [ \"$1\" = env ] && [ \"$2\" = GOROOT ]; then\n  printf '%s\\n' '{}'\nelse\n  exit 9\nfi\n",
                goroot.display()
            );
            fs::write(path, script).expect("write fake go launcher");
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .expect("make fake go executable");
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let cache = test_cache_root(&temp);
        ensure_private_cache_root(&cache).expect("private cache");
        let target = GoHostTarget::current_process()
            .expect("supported host")
            .label();
        let goroot_a = temp.path().join("goroot-a");
        let goroot_b = temp.path().join("goroot-b");
        fs::create_dir(&goroot_a).expect("create goroot A");
        fs::create_dir(&goroot_b).expect("create goroot B");
        let bin_a = goroot_a.join("bin");
        let bin_b = goroot_b.join("bin");
        fs::create_dir(&bin_a).expect("create bin A");
        fs::create_dir(&bin_b).expect("create bin B");
        let go_a = bin_a.join("go");
        let go_b = bin_b.join("go");
        write_launcher(&go_a, "A", &target, &goroot_a);
        write_launcher(&go_b, "B", &target, &goroot_b);
        let path_a = std::env::join_paths([&bin_a]).expect("PATH A");
        let path_b = std::env::join_paths([&bin_b]).expect("PATH B");
        let prepared_go_a =
            prepare_go_toolchain(&cache, local_go_toolchain_in(path_a).expect("toolchain A"))
                .expect("prepare toolchain A");
        let prepared_go_b =
            prepare_go_toolchain(&cache, local_go_toolchain_in(path_b).expect("toolchain B"))
                .expect("prepare toolchain B");
        let frontend = temp.path().join("frontend");
        fs::write(&frontend, "#!/bin/sh\nexec go version\n").expect("write frontend");
        fs::set_permissions(&frontend, fs::Permissions::from_mode(0o700))
            .expect("make frontend executable");
        let prepared_a =
            prepare_binary_frontend(&cache, &frontend, prepared_go_a).expect("prepare frontend A");
        let prepared_b =
            prepare_binary_frontend(&cache, &frontend, prepared_go_b).expect("prepare frontend B");
        write_launcher(&go_a, "REPLACED", &target, &goroot_a);

        let output = prepared_a
            .command(temp.path())
            .output()
            .expect("run prepared frontend");

        assert_ne!(prepared_a.identity_digest(), prepared_b.identity_digest());
        assert!(String::from_utf8_lossy(&output.stdout).contains(" A"));
        make_directory_tree_writable(&cache).expect("reopen cache for cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_flat_file_overflow_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        for index in 0..GO_FRONTEND_MAX_SOURCE_FILES {
            fs::write(
                temp.path().join(format!("source-{index:04}.go")),
                "package main\n",
            )
            .expect("write source");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("file overflow must fail");

        assert!(error.to_string().contains("more than 512 files"));
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_excessive_depth_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        let mut directory = temp.path().to_path_buf();
        for index in 0..=GO_FRONTEND_MAX_SOURCE_DEPTH {
            directory.push(format!("d{index}"));
            fs::create_dir(&directory).expect("create nested directory");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("depth overflow must fail");

        assert!(error.to_string().contains("traversal depth"));
    }

    #[cfg(unix)]
    #[test]
    fn source_snapshot_rejects_irrelevant_entry_overflow_during_walk() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("go.mod"), "module example.test/frontend\n")
            .expect("write go.mod");
        for index in 0..GO_FRONTEND_MAX_SOURCE_ENTRIES {
            fs::write(
                temp.path().join(format!("ignored-{index:04}.txt")),
                "ignored",
            )
            .expect("write irrelevant entry");
        }

        let error = capture_source_snapshot(temp.path()).expect_err("entry overflow must fail");

        assert!(error.to_string().contains("more than 4096 entries"));
    }

    #[cfg(unix)]
    #[test]
    fn staging_guard_cleans_every_partial_permission_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        for state in 0..4 {
            let path;
            {
                let staging = StagingDirectory::create(temp.path(), ".failure-")
                    .expect("create staging directory");
                path = staging.path().to_path_buf();
                if state >= 1 {
                    write_new_private_file(&path.join("file"), b"contents", false)
                        .expect("write staging file");
                }
                if state >= 2 {
                    let nested = ensure_private_subdirectory(&path, Path::new("nested"))
                        .expect("create nested staging directory");
                    write_new_private_file(&nested.join("file"), b"contents", false)
                        .expect("write nested staging file");
                }
                if state >= 3 {
                    seal_execution_directory(&path).expect("seal staging directory");
                }
            }
            assert!(!path.exists(), "staging state {state} leaked");
        }
    }

    #[cfg(unix)]
    #[test]
    fn staging_allocation_is_unique_under_concurrency() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = std::sync::Arc::new(temp.path().to_path_buf());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let mut handles = Vec::new();
        for _ in 0..16 {
            let parent = std::sync::Arc::clone(&parent);
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let staging = StagingDirectory::create(&parent, ".concurrent-")
                    .expect("allocate staging directory");
                let path = staging.path().to_path_buf();
                barrier.wait();
                path
            }));
        }
        let paths = handles
            .into_iter()
            .map(|handle| handle.join().expect("join staging allocator"))
            .collect::<BTreeSet<_>>();

        assert_eq!(paths.len(), 16);
        assert!(paths.iter().all(|path| !path.exists()));
    }

    #[cfg(unix)]
    #[test]
    fn stale_staging_cleanup_is_bounded_and_prefix_scoped() {
        let temp = tempfile::tempdir().expect("tempdir");
        let stale = temp.path().join(".source-stale");
        let unrelated = temp.path().join("published");
        fs::create_dir(&stale).expect("create stale staging directory");
        fs::create_dir(&unrelated).expect("create unrelated directory");

        cleanup_stale_staging_directories(temp.path(), ".source-", std::time::Duration::ZERO);

        assert!(!stale.exists());
        assert!(unrelated.exists());
    }

    #[cfg(unix)]
    #[test]
    fn hostile_fifo_is_rejected_without_blocking() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fifo = temp.path().join("frontend");
        let status = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        let (sender, receiver) = std::sync::mpsc::channel();
        let reader_path = fifo;
        let reader = std::thread::spawn(move || {
            let _ = sender.send(read_regular_file_no_follow(&reader_path));
        });

        let result = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("FIFO rejection must not block");

        assert!(result.is_err());
        reader.join().expect("join FIFO reader");
    }
}
