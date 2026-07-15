use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::module_graph::paths::read_repo_file_anchored_to_string_with_limit;

pub(crate) const ADAPTATION_MODEL_DIR: &str = ".polint/models";
pub(crate) const ADAPTATION_MODEL_MAX_BYTES: u64 = 1_048_576;
const ADAPTATION_MODEL_MAX_VISITED_ENTRIES: usize = 4_096;
const ADAPTATION_MODEL_MAX_VISITED_DIRECTORIES: usize = 256;
const ADAPTATION_MODEL_MAX_PENDING_ENTRIES: usize = 256;

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
    traversal_stats: TraversalStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TraversalStats {
    visited_entries: usize,
    visited_directories: usize,
    peak_pending_entries: usize,
    peak_retained_paths: usize,
}

#[derive(Debug)]
enum DiscoveryEntry {
    Directory(PathBuf),
    File,
}

impl AdaptationModelInventory {
    pub(crate) fn discover(root: &Path, max_model_files: usize) -> Self {
        Self::discover_inner(root, max_model_files, &mut |_| {})
    }

    fn discover_inner(
        root: &Path,
        max_model_files: usize,
        before_read: &mut impl FnMut(&str),
    ) -> Self {
        let mut issues = Vec::new();
        let mut paths = Vec::new();
        let mut budget_exceeded_at = None;
        let mut visited_entries = 0_usize;
        let mut visited_directories = 0_usize;
        let mut peak_pending_entries = 0_usize;
        let mut peak_retained_paths = 0_usize;
        let mut pending_was_truncated = false;
        let mut directory_budget_exceeded_at = None;
        let mut entry_budget_exceeded = false;
        let model_root = root.join(ADAPTATION_MODEL_DIR);
        let metadata = match fs::symlink_metadata(&model_root) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => {
                issues.push(issue(
                    ADAPTATION_MODEL_DIR,
                    "Adaptation model directory could not be read.",
                    "read_error",
                    "metadata unavailable",
                ));
                None
            }
        };

        if let Some(metadata) = metadata {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                issues.push(issue(
                    ADAPTATION_MODEL_DIR,
                    "Adaptation model directory was ignored because it is not a regular directory.",
                    "read_error",
                    "not a directory",
                ));
            } else {
                let mut pending = BTreeMap::from([(
                    ADAPTATION_MODEL_DIR.to_string(),
                    DiscoveryEntry::Directory(model_root),
                )]);
                peak_pending_entries = pending.len();
                'walk: while let Some((relative_path, entry)) = pending.pop_first() {
                    let DiscoveryEntry::Directory(absolute_dir) = entry else {
                        if paths.len() >= max_model_files {
                            budget_exceeded_at = Some(relative_path);
                            break;
                        }
                        paths.push(relative_path);
                        peak_retained_paths = peak_retained_paths.max(paths.len());
                        continue;
                    };
                    if visited_directories >= ADAPTATION_MODEL_MAX_VISITED_DIRECTORIES {
                        directory_budget_exceeded_at = Some(relative_path);
                        break;
                    }
                    visited_directories += 1;
                    let raw_entries = match fs::read_dir(&absolute_dir) {
                        Ok(entries) => entries,
                        Err(_) => {
                            issues.push(issue(
                                &relative_path,
                                "Adaptation model directory entry could not be read.",
                                "read_error",
                                "read_dir failed",
                            ));
                            continue;
                        }
                    };
                    for entry in raw_entries {
                        if visited_entries >= ADAPTATION_MODEL_MAX_VISITED_ENTRIES {
                            entry_budget_exceeded = true;
                            break 'walk;
                        }
                        visited_entries += 1;
                        let entry = match entry {
                            Ok(entry) => entry,
                            Err(_) => {
                                issues.push(issue(
                                    &relative_path,
                                    "Adaptation model directory entry could not be read.",
                                    "read_error",
                                    "directory entry unavailable",
                                ));
                                continue;
                            }
                        };
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        let child_relative_path = format!("{relative_path}/{file_name}");
                        let path = entry.path();
                        let metadata = match fs::symlink_metadata(&path) {
                            Ok(metadata) => metadata,
                            Err(_) => {
                                issues.push(issue(
                                    &child_relative_path,
                                    "Adaptation model path could not be read.",
                                    "read_error",
                                    "metadata unavailable",
                                ));
                                continue;
                            }
                        };
                        if metadata.file_type().is_symlink() {
                            issues.push(issue(
                                &child_relative_path,
                                "Adaptation model path was ignored because symlinks are not allowed.",
                                "read_error",
                                "symlink",
                            ));
                        } else if metadata.is_dir() {
                            pending.insert(child_relative_path, DiscoveryEntry::Directory(path));
                        } else if metadata.is_file()
                            && path
                                .extension()
                                .is_some_and(|extension| extension == "toml")
                        {
                            pending.insert(child_relative_path, DiscoveryEntry::File);
                        }
                        if pending.len() > ADAPTATION_MODEL_MAX_PENDING_ENTRIES {
                            pending.pop_last();
                            pending_was_truncated = true;
                        }
                        peak_pending_entries = peak_pending_entries.max(pending.len());
                    }
                }
            }
        }

        if entry_budget_exceeded {
            paths.clear();
            issues.clear();
            budget_exceeded_at = None;
            issues.push(issue(
                ADAPTATION_MODEL_DIR,
                "Adaptation model discovery stopped because the traversal-entry budget was exceeded.",
                "budget",
                format!("max_visited_entries={ADAPTATION_MODEL_MAX_VISITED_ENTRIES}"),
            ));
        } else {
            if let Some(relative_path) = directory_budget_exceeded_at {
                issues.push(issue(
                    relative_path,
                    "Adaptation model discovery stopped because the directory budget was exceeded.",
                    "budget",
                    format!("max_visited_directories={ADAPTATION_MODEL_MAX_VISITED_DIRECTORIES}"),
                ));
            }
            if pending_was_truncated {
                issues.push(issue(
                    ADAPTATION_MODEL_DIR,
                    "Adaptation model discovery retained only the lexicographically earliest bounded traversal frontier.",
                    "budget",
                    format!("max_pending_entries={ADAPTATION_MODEL_MAX_PENDING_ENTRIES}"),
                ));
            }
        }

        let mut files = Vec::new();
        for relative_path in paths {
            before_read(&relative_path);
            match read_repo_file_anchored_to_string_with_limit(
                root,
                &relative_path,
                ADAPTATION_MODEL_MAX_BYTES,
            ) {
                Ok(contents) => files.push(AdaptationModelFileInput {
                    relative_path,
                    contents,
                }),
                Err(error) => issues.push(issue(
                    &relative_path,
                    format!("Adaptation model file was ignored: {error}"),
                    "read_error",
                    error.stable_reason(),
                )),
            }
        }
        if let Some(relative_path) = &budget_exceeded_at {
            issues.push(issue(
                relative_path,
                "Adaptation model discovery stopped because the model-file budget was exceeded.",
                "budget",
                format!("max_model_files={max_model_files}"),
            ));
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
            traversal_stats: TraversalStats {
                visited_entries,
                visited_directories,
                peak_pending_entries,
                peak_retained_paths,
            },
        }
    }

    pub(crate) fn is_absent(&self) -> bool {
        self.files.is_empty() && self.issues.is_empty()
    }
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
        assert!(inventory.issues.iter().any(|issue| {
            issue.evidence_value
                == format!("max_visited_entries={ADAPTATION_MODEL_MAX_VISITED_ENTRIES}")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn inventory_read_rejects_file_swapped_to_outside_symlink() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let model_root = repo.path().join(ADAPTATION_MODEL_DIR);
        fs::create_dir_all(&model_root).expect("model directory");
        let model = model_root.join("rules.toml");
        fs::write(&model, "schema = 1\n").expect("write model");
        let outside_model = outside.path().join("outside.toml");
        fs::write(&outside_model, "secret = \"outside\"\n").expect("write outside model");

        let inventory = AdaptationModelInventory::discover_inner(repo.path(), 32, &mut |_| {
            fs::remove_file(&model).expect("remove model");
            std::os::unix::fs::symlink(&outside_model, &model).expect("replace with symlink");
        });

        assert!(inventory.files.is_empty());
        assert!(
            inventory
                .issues
                .iter()
                .any(|issue| issue.evidence_value == "path escapes repository root")
        );
    }
}
