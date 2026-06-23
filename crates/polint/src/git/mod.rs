//! Thin `git` shell-out for `polint review`.
//!
//! This module computes the changeset (changed paths, statuses, and new-side
//! line ranges) between `HEAD` and a target ref, for the `polint review`
//! diff gate. It deliberately shells out to the `git` binary via
//! [`std::process::Command`] in the style of [`crate::go::lifecycle`] rather
//! than taking a `git2`/`gix` dependency.
//!
//! The module is crate-private: there is no `polint::git` public path. The
//! produced [`ReviewChangeset`] is injected into the host's `AnalysisDb` so the
//! `ChangedFiles` SDK fact view can read it.
//!
//! Paths are emitted **repo-relative and `/`-normalized**, identical in form to
//! `Diagnostic.file` (`SourceFile.relative_path`, normalized at file ingest in
//! `fs/mod.rs`). The default finding-level diff gate compares diagnostics
//! against these paths, so a normalization mismatch would silently drop
//! findings.

// The whole module is exercised by its own tests in this task; the `polint
// review` command wires `changeset_for_ref` into the host in Task 4.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Wired into the polint review command (and thus non-test call sites) in Task 4."
    )
)]

use crate::core::{ChangeStatus, ChangedFile, ReviewChangeset};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// The `git` binary to invoke, honoring an optional `POLINT_GIT` override.
///
/// Mirrors the `POLINT_CARGO`/`CARGO` override idiom used by the rule-host
/// subprocess so tests can stay hermetic.
fn git_bin() -> String {
    std::env::var("POLINT_GIT").unwrap_or_else(|_| "git".to_string())
}

/// Normalize a git-emitted path to the repo-relative, `/`-normalized form used
/// by `Diagnostic.file`.
///
/// `git diff` already yields repo-relative paths; this strips a leading `./`
/// and converts `\` to `/` so the result compares equal to
/// `SourceFile.relative_path`.
fn normalize_path(raw: &str) -> String {
    let slashed = raw.replace('\\', "/");
    slashed.strip_prefix("./").unwrap_or(&slashed).to_string()
}

/// Run `git <args>` in `root`, returning stdout bytes on success.
///
/// On a non-zero exit, fails loudly with the command and stderr context.
fn run_git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let git = git_bin();
    let output = Command::new(&git)
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to invoke `{git} {}`", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`{git} {}` failed ({}): {}",
            args.join(" "),
            output.status,
            stderr.trim()
        );
    }
    Ok(output.stdout)
}

/// Resolve the diff base for `target` against `HEAD`.
///
/// A three-dot range (`a...b`) is passed straight to `git diff`, which computes
/// the merge-base form natively. Otherwise the merge-base of `target` and
/// `HEAD` is resolved explicitly so the diff has PR semantics (changes on the
/// working side only, not changes the target made since the fork point).
fn resolve_base(root: &Path, target: &str) -> Result<String> {
    if target.contains("...") {
        return Ok(target.to_string());
    }
    let stdout = run_git(root, &["merge-base", target, "HEAD"]).with_context(|| {
        format!("could not resolve merge-base of `{target}` and HEAD — is `{target}` a valid ref?")
    })?;
    let base = String::from_utf8_lossy(&stdout).trim().to_string();
    if base.is_empty() {
        bail!("could not resolve merge-base of `{target}` and HEAD — empty result");
    }
    Ok(base)
}

/// Map a `git diff --name-status` status letter to a [`ChangeStatus`].
///
/// `R###`/`C###` carry a similarity score suffix; only the leading letter is
/// inspected. Copies are treated as `Added` (the new path is genuinely new
/// content on the working side); type-changes (`T`) are `Modified`.
fn status_from_letter(letter: u8) -> ChangeStatus {
    match letter {
        b'A' => ChangeStatus::Added,
        b'D' => ChangeStatus::Deleted,
        b'R' => ChangeStatus::Renamed,
        b'C' => ChangeStatus::Added,
        // 'M', 'T', and any unexpected letter fall back to Modified.
        _ => ChangeStatus::Modified,
    }
}

/// Parse `git diff --name-status -z <base>` output into (path, status) pairs.
///
/// The `-z` format is NUL-delimited and robust to spaces in paths. Plain
/// records are `<status>\0<path>\0`; rename/copy records are
/// `<status>\0<old-path>\0<new-path>\0` (the new path is used).
fn parse_name_status(stdout: &[u8]) -> BTreeMap<String, ChangeStatus> {
    let text = String::from_utf8_lossy(stdout);
    let mut fields = text.split('\0').filter(|field| !field.is_empty());
    let mut out = BTreeMap::new();
    while let Some(status_field) = fields.next() {
        let letter = status_field.as_bytes().first().copied().unwrap_or(b'M');
        let status = status_from_letter(letter);
        let is_rename_or_copy = matches!(letter, b'R' | b'C');
        // Rename/copy records carry two paths; the new (second) path is used.
        let path = if is_rename_or_copy {
            let _old = fields.next();
            fields.next()
        } else {
            fields.next()
        };
        let Some(path) = path else { break };
        out.insert(normalize_path(path), status);
    }
    out
}

/// Parse `@@ -l,s +L,S @@` hunk headers from `git diff --unified=0 <base>` and
/// collect new-side line ranges per file.
///
/// Each file's hunks are associated with the path from the preceding
/// `+++ b/<path>` header. `+++ /dev/null` (a deleted file) contributes no
/// ranges. A hunk with new-side count `0` (a pure-deletion hunk) contributes no
/// new-side range. Ranges are inclusive, 1-based: `(L, L + S - 1)`.
fn parse_new_line_ranges(stdout: &[u8]) -> BTreeMap<String, Vec<(u32, u32)>> {
    let text = String::from_utf8_lossy(stdout);
    let mut ranges: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            current = parse_plus_header(rest);
            // Ensure the file appears even if it has no textual hunks (e.g. a
            // binary or mode-only change still gets an entry from name-status).
            continue;
        }
        if let Some(rest) = line.strip_prefix("@@ ")
            && let Some(path) = current.as_deref()
            && let Some(range) = parse_hunk_new_range(rest)
        {
            ranges.entry(path.to_string()).or_default().push(range);
        }
    }
    ranges
}

/// Extract the new-side path from a `+++ ` header body.
///
/// The body is typically `b/<path>` (optionally followed by a tab and metadata)
/// or `/dev/null` for a deleted file (returns `None`).
fn parse_plus_header(rest: &str) -> Option<String> {
    // Strip any trailing tab-delimited metadata git appends.
    let path_part = rest.split('\t').next().unwrap_or(rest).trim();
    if path_part == "/dev/null" {
        return None;
    }
    let path = path_part.strip_prefix("b/").unwrap_or(path_part);
    Some(normalize_path(path))
}

/// Parse the new-side range from a hunk header body `-l,s +L,S @@ ...`.
///
/// Returns `(L, L + S - 1)` for `S > 0`; `None` for a pure-deletion hunk
/// (`S == 0`). The new-side count defaults to `1` when omitted (`+L @@`).
fn parse_hunk_new_range(rest: &str) -> Option<(u32, u32)> {
    // rest looks like: "-12,0 +13,2 @@ optional context"
    let plus = rest.split('+').nth(1)?;
    let token = plus.split_whitespace().next()?;
    let mut parts = token.splitn(2, ',');
    let start: u32 = parts.next()?.parse().ok()?;
    let count: u32 = match parts.next() {
        Some(count_str) => count_str.parse().ok()?,
        None => 1,
    };
    if count == 0 {
        return None;
    }
    Some((start, start + count - 1))
}

/// Compute the [`ReviewChangeset`] between `HEAD` and `target` for `polint review`.
///
/// `root` is the repo root; `target` is the user's ref (`origin/main`, a SHA,
/// or an `a...b` range). Paths are repo-relative and `/`-normalized to match
/// `Diagnostic.file`. The returned files are sorted by path for deterministic
/// output (the serialized cache file and diff-gated output are stable).
///
/// Edge cases: deleted files carry empty `new_line_ranges`; renames carry the
/// new path with `Renamed`; binary and mode-only changes appear with empty
/// ranges; an empty diff yields an empty file list. A bad ref is a loud `Err`.
pub(crate) fn changeset_for_ref(root: &Path, target: &str) -> Result<ReviewChangeset> {
    let base = resolve_base(root, target)?;

    let name_status = run_git(root, &["diff", "--name-status", "-z", &base])?;
    let statuses = parse_name_status(&name_status);

    let unified = run_git(root, &["diff", "--unified=0", &base])?;
    let mut ranges = parse_new_line_ranges(&unified);

    let mut files: Vec<ChangedFile> = statuses
        .into_iter()
        .map(|(path, status)| {
            let new_line_ranges = match status {
                // Deleted files never carry new-side lines.
                ChangeStatus::Deleted => Vec::new(),
                _ => ranges.remove(&path).unwrap_or_default(),
            };
            ChangedFile {
                path,
                status,
                new_line_ranges,
            }
        })
        .collect();

    // `BTreeMap` already yields sorted keys, but sort defensively so the
    // contract (path-sorted, deterministic) holds regardless of source.
    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(ReviewChangeset { files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    /// Returns `true` when `git` is usable; tests skip cleanly otherwise.
    fn git_available() -> bool {
        Command::new(git_bin())
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new(git_bin())
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git invocation");
        assert!(
            status.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// Initialize a repo with deterministic identity and config.
    fn init_repo() -> TempDir {
        let temp = TempDir::new().expect("tempdir");
        let dir = temp.path();
        git(dir, &["init", "--quiet"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        temp
    }

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, contents).expect("write file");
    }

    fn commit_all(dir: &Path, message: &str) -> String {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", message]);
        let out = Command::new(git_bin())
            .current_dir(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn find<'a>(facts: &'a ReviewChangeset, path: &str) -> &'a ChangedFile {
        facts
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("missing {path} in {:?}", facts.files))
    }

    #[test]
    fn classifies_added_modified_deleted_renamed_with_normalized_paths() {
        if !git_available() {
            eprintln!("skipping git changeset test; `git` not on PATH");
            return;
        }
        let temp = init_repo();
        let dir = temp.path();

        // Base commit: files that will be modified, deleted, and renamed.
        write(
            dir,
            "src/router.go",
            "package app\nfunc a() {}\nfunc b() {}\n",
        );
        write(dir, "src/legacy.go", "package app\nfunc old() {}\n");
        write(dir, "src/old_name.go", "package app\nfunc keep() {}\n");
        let base = commit_all(dir, "base");

        // Mutate the working tree: add, modify, delete, rename.
        write(
            dir,
            "db/migrations/0001_init.sql",
            "CREATE TABLE t (id INT);\n",
        );
        write(
            dir,
            "src/router.go",
            "package app\nfunc a() {}\nfunc b() { println(1) }\n",
        );
        std::fs::remove_file(dir.join("src/legacy.go")).expect("rm");
        git(dir, &["mv", "src/old_name.go", "src/new_name.go"]);
        commit_all(dir, "changes");

        let facts = changeset_for_ref(dir, &base).expect("changeset");
        let paths: Vec<&str> = facts.files.iter().map(|f| f.path.as_str()).collect();

        // Deterministic, path-sorted, `/`-normalized repo-relative paths.
        assert_eq!(
            paths,
            vec![
                "db/migrations/0001_init.sql",
                "src/legacy.go",
                "src/new_name.go",
                "src/router.go",
            ]
        );

        let added = find(&facts, "db/migrations/0001_init.sql");
        assert_eq!(added.status, ChangeStatus::Added);
        assert_eq!(added.new_line_ranges, vec![(1, 1)]);

        let modified = find(&facts, "src/router.go");
        assert_eq!(modified.status, ChangeStatus::Modified);
        // Only line 3 changed (the new-side range is a subset, not the whole file).
        assert_eq!(modified.new_line_ranges, vec![(3, 3)]);

        let deleted = find(&facts, "src/legacy.go");
        assert_eq!(deleted.status, ChangeStatus::Deleted);
        assert!(deleted.new_line_ranges.is_empty());

        let renamed = find(&facts, "src/new_name.go");
        assert_eq!(renamed.status, ChangeStatus::Renamed);
    }

    #[test]
    fn three_dot_range_form_resolves() {
        if !git_available() {
            eprintln!("skipping git changeset test; `git` not on PATH");
            return;
        }
        let temp = init_repo();
        let dir = temp.path();
        write(dir, "a.txt", "one\n");
        let base = commit_all(dir, "base");
        write(dir, "a.txt", "one\ntwo\n");
        let head = commit_all(dir, "head");

        let range = format!("{base}...{head}");
        let facts = changeset_for_ref(dir, &range).expect("changeset");
        let changed = find(&facts, "a.txt");
        assert_eq!(changed.status, ChangeStatus::Modified);
        // Line 2 is the new-side addition.
        assert_eq!(changed.new_line_ranges, vec![(2, 2)]);
    }

    #[test]
    fn empty_diff_yields_no_files() {
        if !git_available() {
            eprintln!("skipping git changeset test; `git` not on PATH");
            return;
        }
        let temp = init_repo();
        let dir = temp.path();
        write(dir, "a.txt", "stable\n");
        let head = commit_all(dir, "base");
        // Diff HEAD against itself: nothing changed.
        let facts = changeset_for_ref(dir, &head).expect("changeset");
        assert!(facts.files.is_empty());
    }

    #[test]
    fn binary_file_change_has_empty_ranges() {
        if !git_available() {
            eprintln!("skipping git changeset test; `git` not on PATH");
            return;
        }
        let temp = init_repo();
        let dir = temp.path();
        std::fs::write(dir.join("blob.bin"), [0u8, 159, 146, 150]).expect("write");
        let base = commit_all(dir, "base");
        std::fs::write(dir.join("blob.bin"), [1u8, 2, 3, 0, 255]).expect("write");
        commit_all(dir, "change binary");

        let facts = changeset_for_ref(dir, &base).expect("changeset");
        let blob = find(&facts, "blob.bin");
        assert_eq!(blob.status, ChangeStatus::Modified);
        assert!(blob.new_line_ranges.is_empty());
    }

    #[test]
    fn bad_ref_is_a_loud_error() {
        if !git_available() {
            eprintln!("skipping git changeset test; `git` not on PATH");
            return;
        }
        let temp = init_repo();
        let dir = temp.path();
        write(dir, "a.txt", "x\n");
        commit_all(dir, "base");
        let err = changeset_for_ref(dir, "definitely-not-a-ref").unwrap_err();
        let message = format!("{err:#}");
        assert!(
            message.contains("definitely-not-a-ref"),
            "error should name the bad ref, got: {message}"
        );
    }
}
