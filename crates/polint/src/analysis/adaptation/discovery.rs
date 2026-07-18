use std::collections::BTreeMap;
use std::path::Path;

#[cfg(test)]
use std::fs;

use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::repo_fs::{
    RepoDirectory, RepoDirectoryEntry, RepoDirectoryEntryKind, RepoFile, RepoFileReadError,
};

pub(crate) const ADAPTATION_MODEL_DIR: &str = ".polint/models";
pub(crate) const ADAPTATION_MODEL_MAX_BYTES: u64 = 1_048_576;
const ADAPTATION_MODEL_MAX_VISITED_ENTRIES: usize = 4_096;
const ADAPTATION_MODEL_MAX_VISITED_DIRECTORIES: usize = 256;
const ADAPTATION_MODEL_MAX_PENDING_ENTRIES: usize = 256;
const ADAPTATION_MODEL_MAX_DEPTH: usize = 64;
const ADAPTATION_MODEL_MAX_PATH_BYTES: usize = 768;
const ADAPTATION_MODEL_MAX_RETAINED_PATH_BYTES: usize = 32 * 1_024;
const ADAPTATION_MODEL_MAX_ISSUES: usize = 128;
const ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE: &str = "bounded_traversal_limit_exceeded";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdaptationModelFileInput {
    pub(crate) relative_path: String,
    pub(crate) contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdaptationModelDiscoveryIssue {
    pub(crate) relative_path: String,
    pub(crate) message: String,
    pub(crate) evidence_key: &'static str,
    pub(crate) evidence_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdaptationModelInventory {
    pub(crate) files: Vec<AdaptationModelFileInput>,
    pub(crate) issues: Vec<AdaptationModelDiscoveryIssue>,
    pub(crate) budget_exceeded_at: Option<String>,
    pub(crate) digest: Digest,
    unsupported: bool,
    traversal_stats: TraversalStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TraversalStats {
    visited_entries: usize,
    visited_directories: usize,
    peak_pending_entries: usize,
    peak_retained_paths: usize,
    peak_retained_path_bytes: usize,
}

#[derive(Debug)]
enum DiscoveryEntryKind {
    Directory(RepoDirectory),
    File(RepoFile),
}

#[derive(Debug)]
struct DiscoveryEntry {
    depth: usize,
    kind: DiscoveryEntryKind,
}

#[derive(Clone, Copy, Debug)]
enum TraversalFailure {
    VisitedEntryBudget,
    VisitedDirectoryBudget,
    DepthBudget,
    PathBytesBudget,
    RetainedPathBytesBudget,
    IssueBudget,
    ResourceExhausted,
    DirectoryEnumeration,
}

impl TraversalFailure {
    fn issue(self) -> AdaptationModelDiscoveryIssue {
        let (message, evidence_key, evidence_value) = match self {
            Self::VisitedEntryBudget
            | Self::VisitedDirectoryBudget
            | Self::DepthBudget
            | Self::PathBytesBudget
            | Self::RetainedPathBytesBudget
            | Self::IssueBudget => (
                "Adaptation model discovery stopped because a bounded traversal safety limit was exceeded.",
                "budget",
                ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE,
            ),
            Self::ResourceExhausted => (
                "Adaptation model discovery stopped because filesystem resources were exhausted.",
                "read_error",
                "filesystem resources exhausted",
            ),
            Self::DirectoryEnumeration => (
                "Adaptation model discovery stopped because a directory stream failed.",
                "read_error",
                "directory enumeration failed",
            ),
        };
        issue(ADAPTATION_MODEL_DIR, message, evidence_key, evidence_value)
    }
}

impl AdaptationModelInventory {
    pub(crate) fn discover(root: &Path, max_model_files: usize) -> Self {
        Self::discover_inner(root, max_model_files, &mut |_| Ok(()), &mut |_| Ok(()))
    }

    fn discover_inner(
        root: &Path,
        max_model_files: usize,
        before_directory_read: &mut impl FnMut(&str) -> Result<(), RepoFileReadError>,
        before_file_read: &mut impl FnMut(&str) -> Result<(), RepoFileReadError>,
    ) -> Self {
        let mut issues = Vec::new();
        let mut selected_files = Vec::new();
        let mut budget_exceeded_at = None;
        let mut visited_entries = 0_usize;
        let mut visited_directories = 0_usize;
        let mut peak_pending_entries = 0_usize;
        let mut peak_retained_paths = 0_usize;
        let mut peak_retained_path_bytes = 0_usize;
        let mut pending_was_truncated = false;
        let mut traversal_failure = None;
        let mut unsupported = false;
        let model_root = match RepoDirectory::open(root, Path::new(ADAPTATION_MODEL_DIR)) {
            Ok(directory) => Some(directory),
            Err(error) if error.is_not_found() => None,
            Err(error) if error.is_resource_exhausted() => {
                traversal_failure = Some(TraversalFailure::ResourceExhausted);
                None
            }
            Err(error) => {
                unsupported = error.is_secure_open_unavailable();
                issues.push(issue(
                    ADAPTATION_MODEL_DIR,
                    format!("Adaptation model directory was ignored: {error}"),
                    "read_error",
                    error.stable_reason(),
                ));
                None
            }
        };

        if let Some(model_root) = model_root {
            let mut retained_path_bytes = ADAPTATION_MODEL_DIR.len();
            peak_retained_path_bytes = retained_path_bytes;
            let mut pending = BTreeMap::from([(
                ADAPTATION_MODEL_DIR.to_string(),
                DiscoveryEntry {
                    depth: 0,
                    kind: DiscoveryEntryKind::Directory(model_root),
                },
            )]);
            peak_pending_entries = pending.len();
            'walk: while let Some((relative_path, entry)) = pending.pop_first() {
                let entry_depth = entry.depth;
                let mut directory = match entry.kind {
                    DiscoveryEntryKind::File(file) => {
                        if selected_files.len() >= max_model_files {
                            budget_exceeded_at = Some(relative_path);
                            break;
                        }
                        selected_files.push((relative_path, file));
                        peak_retained_paths = peak_retained_paths.max(selected_files.len());
                        continue;
                    }
                    DiscoveryEntryKind::Directory(directory) => directory,
                };
                if visited_directories >= ADAPTATION_MODEL_MAX_VISITED_DIRECTORIES {
                    traversal_failure = Some(TraversalFailure::VisitedDirectoryBudget);
                    break;
                }
                visited_directories += 1;
                if let Err(error) = before_directory_read(&relative_path) {
                    traversal_failure = Some(directory_failure(&error));
                    break;
                }
                let mut saw_non_unicode_name = false;
                let visit_result = directory.visit_entries(|entry: RepoDirectoryEntry| {
                    if let Err(error) = &entry.kind
                        && error.is_resource_exhausted()
                    {
                        traversal_failure = Some(TraversalFailure::ResourceExhausted);
                        return false;
                    }
                    if visited_entries >= ADAPTATION_MODEL_MAX_VISITED_ENTRIES {
                        traversal_failure = Some(TraversalFailure::VisitedEntryBudget);
                        return false;
                    }
                    visited_entries += 1;
                    let Some(file_name) = entry.name.to_str() else {
                        saw_non_unicode_name = true;
                        return true;
                    };
                    if entry.kind.is_err() && issues.len() >= ADAPTATION_MODEL_MAX_ISSUES {
                        traversal_failure = Some(TraversalFailure::IssueBudget);
                        return false;
                    }
                    let child_depth = match entry_depth.checked_add(1) {
                        Some(depth) if depth <= ADAPTATION_MODEL_MAX_DEPTH => depth,
                        _ => {
                            traversal_failure = Some(TraversalFailure::DepthBudget);
                            return false;
                        }
                    };
                    let child_path_bytes = match relative_path
                        .len()
                        .checked_add(1)
                        .and_then(|length| length.checked_add(file_name.len()))
                    {
                        Some(length) if length <= ADAPTATION_MODEL_MAX_PATH_BYTES => length,
                        _ => {
                            traversal_failure = Some(TraversalFailure::PathBytesBudget);
                            return false;
                        }
                    };
                    let Some(next_retained_path_bytes) = retained_path_bytes
                        .checked_add(child_path_bytes)
                        .filter(|length| *length <= ADAPTATION_MODEL_MAX_RETAINED_PATH_BYTES)
                    else {
                        traversal_failure = Some(TraversalFailure::RetainedPathBytesBudget);
                        return false;
                    };
                    let mut child_relative_path = String::with_capacity(child_path_bytes);
                    child_relative_path.push_str(&relative_path);
                    child_relative_path.push('/');
                    child_relative_path.push_str(file_name);
                    retained_path_bytes = next_retained_path_bytes;
                    peak_retained_path_bytes = peak_retained_path_bytes.max(retained_path_bytes);
                    let retain_child_path = match entry.kind {
                        Err(error) => {
                            issues.push(issue(
                                child_relative_path,
                                "Adaptation model path was ignored.",
                                "read_error",
                                error.stable_reason(),
                            ));
                            true
                        }
                        Ok(RepoDirectoryEntryKind::Directory(directory)) => {
                            let replaced = pending.insert(
                                child_relative_path,
                                DiscoveryEntry {
                                    depth: child_depth,
                                    kind: DiscoveryEntryKind::Directory(directory),
                                },
                            );
                            if replaced.is_some() {
                                retained_path_bytes =
                                    retained_path_bytes.saturating_sub(child_path_bytes);
                            }
                            true
                        }
                        Ok(RepoDirectoryEntryKind::File(file))
                            if Path::new(file_name)
                                .extension()
                                .is_some_and(|extension| extension == "toml") =>
                        {
                            let replaced = pending.insert(
                                child_relative_path,
                                DiscoveryEntry {
                                    depth: child_depth,
                                    kind: DiscoveryEntryKind::File(file),
                                },
                            );
                            if replaced.is_some() {
                                retained_path_bytes =
                                    retained_path_bytes.saturating_sub(child_path_bytes);
                            }
                            true
                        }
                        Ok(RepoDirectoryEntryKind::File(_)) | Ok(RepoDirectoryEntryKind::Other) => {
                            false
                        }
                    };
                    if !retain_child_path {
                        retained_path_bytes = retained_path_bytes.saturating_sub(child_path_bytes);
                    }
                    if pending.len() > ADAPTATION_MODEL_MAX_PENDING_ENTRIES {
                        if let Some((dropped_path, _)) = pending.pop_last() {
                            retained_path_bytes =
                                retained_path_bytes.saturating_sub(dropped_path.len());
                        }
                        pending_was_truncated = true;
                    }
                    peak_pending_entries = peak_pending_entries.max(pending.len());
                    true
                });
                if let Err(error) = visit_result {
                    traversal_failure = Some(directory_failure(&error));
                    break;
                }
                if traversal_failure.is_some() {
                    break 'walk;
                }
                if saw_non_unicode_name {
                    if let Err(failure) = push_cloned_path_issue(
                        &mut issues,
                        &relative_path,
                        "Adaptation model directory contained an entry with a non-Unicode name.",
                        "read_error",
                        "non-unicode entry name",
                        &mut retained_path_bytes,
                    ) {
                        traversal_failure = Some(failure);
                        break;
                    }
                    peak_retained_path_bytes = peak_retained_path_bytes.max(retained_path_bytes);
                }
                retained_path_bytes = retained_path_bytes.saturating_sub(relative_path.len());
            }
            retained_path_bytes =
                retained_path_bytes_for(&selected_files, &issues, budget_exceeded_at.as_deref());
            if traversal_failure.is_none() && pending_was_truncated {
                match reserve_issue_path(&issues, ADAPTATION_MODEL_DIR, &mut retained_path_bytes) {
                    Ok(relative_path) => {
                        peak_retained_path_bytes =
                            peak_retained_path_bytes.max(retained_path_bytes);
                        issues.push(issue(
                            relative_path,
                            "Adaptation model discovery retained only the lexicographically earliest bounded traversal frontier.",
                            "budget",
                            format!("max_pending_entries={ADAPTATION_MODEL_MAX_PENDING_ENTRIES}"),
                        ));
                    }
                    Err(failure) => traversal_failure = Some(failure),
                }
            }
        }

        let mut files = Vec::new();
        for (relative_path, file) in selected_files {
            if traversal_failure.is_some() {
                break;
            }
            if let Err(error) = before_file_read(&relative_path) {
                if error.is_resource_exhausted() {
                    traversal_failure = Some(TraversalFailure::ResourceExhausted);
                    break;
                }
                if let Err(failure) = push_counted_path_issue(
                    &mut issues,
                    relative_path,
                    "Adaptation model file was ignored.",
                    "read_error",
                    error.stable_reason(),
                ) {
                    traversal_failure = Some(failure);
                }
                continue;
            }
            match file.read_to_string_with_limit(ADAPTATION_MODEL_MAX_BYTES) {
                Ok(contents) => files.push(AdaptationModelFileInput {
                    relative_path,
                    contents,
                }),
                Err(error) if error.is_resource_exhausted() => {
                    traversal_failure = Some(TraversalFailure::ResourceExhausted);
                }
                Err(error) => {
                    if let Err(failure) = push_counted_path_issue(
                        &mut issues,
                        relative_path,
                        "Adaptation model file was ignored.",
                        "read_error",
                        error.stable_reason(),
                    ) {
                        traversal_failure = Some(failure);
                    }
                }
            }
        }
        if traversal_failure.is_none()
            && let Some(relative_path) = &budget_exceeded_at
        {
            let mut retained_path_bytes =
                retained_path_bytes_for(&files, &issues, budget_exceeded_at.as_deref());
            match reserve_issue_path(&issues, relative_path, &mut retained_path_bytes) {
                Ok(relative_path) => {
                    peak_retained_path_bytes = peak_retained_path_bytes.max(retained_path_bytes);
                    issues.push(issue(
                        relative_path,
                        "Adaptation model discovery stopped because the model-file budget was exceeded.",
                        "budget",
                        format!("max_model_files={max_model_files}"),
                    ));
                }
                Err(failure) => traversal_failure = Some(failure),
            }
        }
        if let Some(failure) = traversal_failure {
            files.clear();
            issues.clear();
            budget_exceeded_at = None;
            issues.push(failure.issue());
        }
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        issues.sort_by(|left, right| {
            (&left.relative_path, left.evidence_key, &left.evidence_value).cmp(&(
                &right.relative_path,
                right.evidence_key,
                &right.evidence_value,
            ))
        });
        let digest = inventory_digest(&files, &issues, budget_exceeded_at.as_deref());
        Self {
            files,
            issues,
            budget_exceeded_at,
            digest,
            unsupported,
            traversal_stats: TraversalStats {
                visited_entries,
                visited_directories,
                peak_pending_entries,
                peak_retained_paths,
                peak_retained_path_bytes,
            },
        }
    }

    pub(crate) fn is_absent(&self) -> bool {
        self.files.is_empty() && self.issues.is_empty()
    }

    pub(crate) fn is_unsupported(&self) -> bool {
        self.unsupported
    }
}

fn directory_failure(error: &RepoFileReadError) -> TraversalFailure {
    if error.is_resource_exhausted() {
        TraversalFailure::ResourceExhausted
    } else {
        TraversalFailure::DirectoryEnumeration
    }
}

fn push_counted_path_issue(
    issues: &mut Vec<AdaptationModelDiscoveryIssue>,
    relative_path: String,
    message: &'static str,
    evidence_key: &'static str,
    evidence_value: &'static str,
) -> Result<(), TraversalFailure> {
    if issues.len() >= ADAPTATION_MODEL_MAX_ISSUES {
        return Err(TraversalFailure::IssueBudget);
    }
    issues.push(issue(relative_path, message, evidence_key, evidence_value));
    Ok(())
}

fn push_cloned_path_issue(
    issues: &mut Vec<AdaptationModelDiscoveryIssue>,
    relative_path: &str,
    message: &'static str,
    evidence_key: &'static str,
    evidence_value: &'static str,
    retained_path_bytes: &mut usize,
) -> Result<(), TraversalFailure> {
    let relative_path = reserve_issue_path(issues, relative_path, retained_path_bytes)?;
    issues.push(issue(relative_path, message, evidence_key, evidence_value));
    Ok(())
}

fn reserve_issue_path(
    issues: &[AdaptationModelDiscoveryIssue],
    relative_path: &str,
    retained_path_bytes: &mut usize,
) -> Result<String, TraversalFailure> {
    if issues.len() >= ADAPTATION_MODEL_MAX_ISSUES {
        return Err(TraversalFailure::IssueBudget);
    }
    let Some(next_retained_path_bytes) = retained_path_bytes
        .checked_add(relative_path.len())
        .filter(|length| *length <= ADAPTATION_MODEL_MAX_RETAINED_PATH_BYTES)
    else {
        return Err(TraversalFailure::RetainedPathBytesBudget);
    };
    *retained_path_bytes = next_retained_path_bytes;
    Ok(relative_path.to_owned())
}

trait RetainedPath {
    fn retained_path(&self) -> &str;
}

impl RetainedPath for (String, RepoFile) {
    fn retained_path(&self) -> &str {
        &self.0
    }
}

impl RetainedPath for AdaptationModelFileInput {
    fn retained_path(&self) -> &str {
        &self.relative_path
    }
}

fn retained_path_bytes_for<T: RetainedPath>(
    files: &[T],
    issues: &[AdaptationModelDiscoveryIssue],
    budget_exceeded_at: Option<&str>,
) -> usize {
    files
        .iter()
        .map(|file| file.retained_path().len())
        .chain(issues.iter().map(|issue| issue.relative_path.len()))
        .chain(budget_exceeded_at.into_iter().map(str::len))
        .fold(0_usize, usize::saturating_add)
}

fn issue(
    relative_path: impl Into<String>,
    message: impl Into<String>,
    evidence_key: &'static str,
    evidence_value: impl Into<String>,
) -> AdaptationModelDiscoveryIssue {
    AdaptationModelDiscoveryIssue {
        relative_path: relative_path.into(),
        message: message.into(),
        evidence_key,
        evidence_value: evidence_value.into(),
    }
}

fn inventory_digest(
    files: &[AdaptationModelFileInput],
    issues: &[AdaptationModelDiscoveryIssue],
    budget_exceeded_at: Option<&str>,
) -> Digest {
    if files.is_empty() && issues.is_empty() {
        return Digest::absent(DigestKind::ModelFile, "model.files");
    }
    let mut parts = Vec::new();
    for file in files {
        parts.push(format!(
            "file={}:content={}",
            file.relative_path,
            crate::cache::stable_hash(&[file.contents.as_str()])
        ));
    }
    for issue in issues {
        parts.push(format!(
            "issue={}:{}={}",
            issue.relative_path, issue.evidence_key, issue.evidence_value
        ));
    }
    if let Some(relative_path) = budget_exceeded_at {
        parts.push(format!("budget_exceeded_at={relative_path}"));
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ModelFile, "model.files", &refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn inventory_digest_changes_on_add_edit_and_delete() {
        let temp = tempfile::tempdir().expect("tempdir");
        let absent = AdaptationModelInventory::discover(temp.path(), 32);
        assert!(absent.is_absent());

        fs::create_dir_all(temp.path().join(ADAPTATION_MODEL_DIR)).expect("model directory");
        let path = temp.path().join(ADAPTATION_MODEL_DIR).join("rules.toml");
        fs::write(&path, "schema = 1\n").expect("write model");
        let added = AdaptationModelInventory::discover(temp.path(), 32);
        fs::write(&path, "schema = 2\n").expect("edit model");
        let edited = AdaptationModelInventory::discover(temp.path(), 32);
        fs::remove_file(path).expect("delete model");
        let deleted = AdaptationModelInventory::discover(temp.path(), 32);

        assert_ne!(absent.digest, added.digest);
        assert_ne!(added.digest, edited.digest);
        assert_eq!(deleted.digest, absent.digest);
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn large_flat_inventory_keeps_frontier_and_retained_paths_bounded() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        for index in 0..1_000 {
            fs::write(
                model_root.join(format!("model-{index:04}.toml")),
                "schema = 1\n",
            )
            .expect("write model");
        }

        let inventory = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(
            inventory.traversal_stats.peak_pending_entries <= ADAPTATION_MODEL_MAX_PENDING_ENTRIES
        );
        assert!(inventory.traversal_stats.peak_retained_paths <= 32);
        assert_eq!(inventory.files.len(), 32);
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn traversal_entry_ceiling_fails_closed_without_order_dependent_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        for index in 0..=ADAPTATION_MODEL_MAX_VISITED_ENTRIES {
            fs::write(
                model_root.join(format!("model-{index:05}.toml")),
                "schema = 1\n",
            )
            .expect("write model");
        }

        let inventory = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(inventory.files.is_empty());
        assert_eq!(
            inventory.traversal_stats.visited_entries,
            ADAPTATION_MODEL_MAX_VISITED_ENTRIES
        );
        assert_fail_closed(inventory, ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE);
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn traversal_depth_budget_fails_closed_with_root_scoped_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut directory = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&directory).expect("model directory");
        for _ in 0..=ADAPTATION_MODEL_MAX_DEPTH {
            directory.push("d");
            fs::create_dir(&directory).expect("nested directory");
        }

        assert_fail_closed(
            AdaptationModelInventory::discover(temp.path(), 32),
            ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE,
        );
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn long_nested_path_fails_before_path_retention() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut directory = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&directory).expect("model directory");
        let component = "x".repeat(170);
        for _ in 0..5 {
            directory.push(&component);
            fs::create_dir(&directory).expect("long-name directory");
        }

        let inventory = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(
            inventory.traversal_stats.peak_retained_path_bytes
                <= ADAPTATION_MODEL_MAX_RETAINED_PATH_BYTES
        );
        assert_fail_closed(inventory, ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE);
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn retained_path_byte_budget_fails_closed_before_frontier_growth() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        for index in 0..220 {
            let name = format!("{}-{index:03}.toml", "m".repeat(160));
            fs::write(model_root.join(name), "schema = 1\n").expect("write model");
        }

        let inventory = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(
            inventory.traversal_stats.peak_retained_path_bytes
                <= ADAPTATION_MODEL_MAX_RETAINED_PATH_BYTES
        );
        assert_fail_closed(inventory, ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE);
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn many_rejected_entries_fail_closed_at_the_issue_budget() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        for index in 0..=ADAPTATION_MODEL_MAX_ISSUES {
            std::os::unix::fs::symlink(
                "missing",
                model_root.join(format!("rejected-{index:03}.toml")),
            )
            .expect("create symlink");
        }

        assert_fail_closed(
            AdaptationModelInventory::discover(temp.path(), 32),
            ADAPTATION_MODEL_TRAVERSAL_BUDGET_EVIDENCE,
        );
    }

    #[test]
    fn traversal_budget_failure_identity_is_independent_of_first_limit() {
        let failures = [
            TraversalFailure::VisitedEntryBudget,
            TraversalFailure::VisitedDirectoryBudget,
            TraversalFailure::DepthBudget,
            TraversalFailure::PathBytesBudget,
            TraversalFailure::RetainedPathBytesBudget,
            TraversalFailure::IssueBudget,
        ];
        let expected_issue = failures[0].issue();
        let expected_digest = inventory_digest(&[], std::slice::from_ref(&expected_issue), None);

        for failure in failures.into_iter().skip(1) {
            let issue = failure.issue();
            assert_eq!(issue, expected_issue);
            assert_eq!(
                inventory_digest(&[], std::slice::from_ref(&issue), None),
                expected_digest
            );
        }
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn directory_stream_failure_clears_already_selected_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(model_root.join("z-directory")).expect("model directories");
        fs::write(model_root.join("a.toml"), "schema = 1\n").expect("write model");

        let inventory = AdaptationModelInventory::discover_inner(
            temp.path(),
            32,
            &mut |relative_path| {
                if relative_path.ends_with("/z-directory") {
                    Err(RepoFileReadError::Read)
                } else {
                    Ok(())
                }
            },
            &mut |_| Ok(()),
        );

        assert_fail_closed(inventory, "directory enumeration failed");
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn resource_exhaustion_during_reads_clears_partial_inventory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        fs::write(model_root.join("a.toml"), "schema = 1\n").expect("write a");
        fs::write(model_root.join("b.toml"), "schema = 2\n").expect("write b");

        let inventory = AdaptationModelInventory::discover_inner(
            temp.path(),
            32,
            &mut |_| Ok(()),
            &mut |relative_path| {
                if relative_path.ends_with("/b.toml") {
                    Err(RepoFileReadError::ResourceExhausted)
                } else {
                    Ok(())
                }
            },
        );

        assert_fail_closed(inventory, "filesystem resources exhausted");
    }

    #[cfg(windows)]
    #[test]
    fn absent_and_repeatedly_enumerated_empty_directories_share_the_absent_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let absent = AdaptationModelInventory::discover(temp.path(), 32);
        fs::create_dir_all(temp.path().join(ADAPTATION_MODEL_DIR)).expect("model directory");
        let first_empty = AdaptationModelInventory::discover(temp.path(), 32);
        let second_empty = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(absent.is_absent());
        assert!(first_empty.is_absent());
        assert!(second_empty.is_absent());
        assert_eq!(absent.digest, first_empty.digest);
        assert_eq!(first_empty.digest, second_empty.digest);
    }

    #[cfg(not(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    #[test]
    fn fallback_treats_absent_optional_directory_as_absent_but_rejects_present_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let absent = AdaptationModelInventory::discover(temp.path(), 32);
        assert!(absent.is_absent());

        fs::create_dir_all(temp.path().join(ADAPTATION_MODEL_DIR)).expect("model directory");
        let present = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(present.files.is_empty());
        assert_eq!(present.issues.len(), 1);
        assert_eq!(
            present.issues[0].evidence_value,
            "secure anchored file reads unavailable"
        );
    }

    #[cfg(any(
        windows,
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn oversized_model_is_rejected_from_the_pinned_file_handle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_root = temp.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        fs::write(
            model_root.join("oversized.toml"),
            vec![b'x'; ADAPTATION_MODEL_MAX_BYTES as usize + 1],
        )
        .expect("write oversized model");

        let inventory = AdaptationModelInventory::discover(temp.path(), 32);

        assert!(inventory.files.is_empty());
        assert!(inventory.issues.iter().any(|issue| {
            issue.relative_path == ".polint/models/oversized.toml"
                && issue.evidence_value == "file exceeds topology input size limit"
        }));
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn inventory_reads_the_pinned_file_after_its_path_is_swapped() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let model_root = repo.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        let model = model_root.join("rules.toml");
        fs::write(&model, "schema = 1\n").expect("write model");
        let outside_model = outside.path().join("outside.toml");
        fs::write(&outside_model, "secret = \"outside\"\n").expect("write outside model");

        let inventory =
            AdaptationModelInventory::discover_inner(repo.path(), 32, &mut |_| Ok(()), &mut |_| {
                fs::remove_file(&model).expect("remove model");
                std::os::unix::fs::symlink(&outside_model, &model).expect("replace with symlink");
                Ok(())
            });

        assert_eq!(
            inventory.files,
            [AdaptationModelFileInput {
                relative_path: ".polint/models/rules.toml".to_string(),
                contents: "schema = 1\n".to_string(),
            }]
        );
        assert!(inventory.issues.is_empty());
        assert!(!format!("{inventory:?}").contains("secret"));
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn inventory_enumerates_the_pinned_directory_after_path_replacement() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let model_root = repo.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        fs::write(model_root.join("rules.toml"), "schema = 1\n").expect("write model");
        fs::write(outside.path().join("secret-name.toml"), "secret = true\n")
            .expect("write outside model");
        let pinned_location = repo.path().join(".polint/models-pinned");
        let mut replaced = false;

        let inventory = AdaptationModelInventory::discover_inner(
            repo.path(),
            32,
            &mut |relative_path| {
                if !replaced && relative_path == ADAPTATION_MODEL_DIR {
                    fs::rename(&model_root, &pinned_location).expect("move pinned directory");
                    std::os::unix::fs::symlink(outside.path(), &model_root)
                        .expect("replace directory path");
                    replaced = true;
                }
                Ok(())
            },
            &mut |_| Ok(()),
        );

        assert_eq!(
            inventory.files,
            [AdaptationModelFileInput {
                relative_path: ".polint/models/rules.toml".to_string(),
                contents: "schema = 1\n".to_string(),
            }]
        );
        let debug = format!("{inventory:?}");
        assert!(!debug.contains("secret-name"));
        assert!(!debug.contains("secret = true"));
    }

    #[cfg(windows)]
    #[test]
    fn inventory_enumerates_the_pinned_directory_after_junction_replacement() {
        use std::process::Command;

        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let model_root = repo.path().join(".polint").join("models");
        fs::create_dir_all(&model_root).expect("model directory");
        fs::write(model_root.join("rules.toml"), "schema = 1\n").expect("write model");
        fs::write(outside.path().join("secret-name.toml"), "secret = true\n")
            .expect("write outside model");
        let pinned_location = repo.path().join(".polint/models-pinned");
        let mut replaced = false;

        let inventory = AdaptationModelInventory::discover_inner(
            repo.path(),
            32,
            &mut |relative_path| {
                if !replaced && relative_path == ADAPTATION_MODEL_DIR {
                    fs::rename(&model_root, &pinned_location).expect("move pinned directory");
                    let output = Command::new("cmd")
                        .args(["/C", "mklink", "/J"])
                        .arg(&model_root)
                        .arg(outside.path())
                        .output()
                        .expect("run mklink");
                    assert!(
                        output.status.success(),
                        "junction creation failed: stdout={} stderr={}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    replaced = true;
                }
                Ok(())
            },
            &mut |_| Ok(()),
        );

        assert_eq!(
            inventory.files,
            [AdaptationModelFileInput {
                relative_path: ".polint/models/rules.toml".to_string(),
                contents: "schema = 1\n".to_string(),
            }]
        );
        let debug = format!("{inventory:?}");
        assert!(!debug.contains("secret-name"));
        assert!(!debug.contains("secret = true"));
    }

    #[cfg(windows)]
    #[test]
    fn inventory_reads_the_pinned_file_after_path_replacement() {
        let repo = tempfile::tempdir().expect("repo");
        let model_root = repo.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        let model = model_root.join("rules.toml");
        let pinned_model = model_root.join("rules-pinned.toml");
        fs::write(&model, "schema = 1\n").expect("write model");

        let inventory =
            AdaptationModelInventory::discover_inner(repo.path(), 32, &mut |_| Ok(()), &mut |_| {
                fs::rename(&model, &pinned_model).expect("move pinned model path");
                fs::write(&model, "secret = true\n").expect("replace model path");
                Ok(())
            });

        assert_eq!(
            inventory.files,
            [AdaptationModelFileInput {
                relative_path: ".polint/models/rules.toml".to_string(),
                contents: "schema = 1\n".to_string(),
            }]
        );
        assert!(!format!("{inventory:?}").contains("secret"));
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn inventory_rejects_root_and_intermediate_symlink_traversal() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        fs::create_dir_all(outside.path().join("models")).expect("outside models");
        fs::write(
            outside.path().join("models/secret-name.toml"),
            "secret = true\n",
        )
        .expect("outside model");
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".polint"))
            .expect("intermediate symlink");

        let intermediate = AdaptationModelInventory::discover(repo.path(), 32);

        assert!(intermediate.files.is_empty());
        assert!(
            intermediate
                .issues
                .iter()
                .any(|issue| issue.evidence_value == "path escapes repository root")
        );
        assert!(!format!("{intermediate:?}").contains("secret-name"));

        let container = tempfile::tempdir().expect("container");
        let root_link = container.path().join("repo-link");
        std::os::unix::fs::symlink(outside.path(), &root_link).expect("root symlink");
        let root = AdaptationModelInventory::discover(&root_link, 32);

        assert!(root.files.is_empty());
        assert!(
            root.issues
                .iter()
                .any(|issue| issue.evidence_value == "path escapes repository root")
        );
        assert!(!format!("{root:?}").contains("secret-name"));
    }

    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn non_unicode_names_produce_parent_scoped_order_invariant_evidence() {
        use std::os::unix::ffi::OsStringExt;

        fn create_inventory(names: [&[u8]; 2]) -> AdaptationModelInventory {
            let repo = tempfile::tempdir().expect("repo");
            let model_root = repo.path().join(ADAPTATION_MODEL_DIR);
            fs::create_dir_all(&model_root).expect("model directory");
            for name in names {
                fs::write(
                    model_root.join(std::ffi::OsString::from_vec(name.to_vec())),
                    "schema = 1\n",
                )
                .expect("write non-Unicode model");
            }
            AdaptationModelInventory::discover(repo.path(), 32)
        }

        let first = create_inventory([b"first-\x80.toml", b"second-\x81.toml"]);
        let second = create_inventory([b"second-\x81.toml", b"first-\x80.toml"]);

        assert_eq!(first.files, second.files);
        assert_eq!(first.issues, second.issues);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.issues.len(), 1);
        assert_eq!(first.issues[0].relative_path, ADAPTATION_MODEL_DIR);
        assert_eq!(first.issues[0].evidence_value, "non-unicode entry name");
        assert!(!format!("{first:?}").contains('\u{fffd}'));
    }

    fn assert_fail_closed(inventory: AdaptationModelInventory, evidence_value: &str) {
        assert!(inventory.files.is_empty());
        assert!(inventory.budget_exceeded_at.is_none());
        assert_eq!(inventory.issues.len(), 1);
        assert_eq!(inventory.issues[0].relative_path, ADAPTATION_MODEL_DIR);
        assert_eq!(inventory.issues[0].evidence_value, evidence_value);
    }
}
