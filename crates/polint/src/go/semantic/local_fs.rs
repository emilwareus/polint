#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
use std::collections::VecDeque;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum LocalFilesystemError {
    #[error("could not establish filesystem locality for `{path}`: {source}")]
    Inspection {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("refusing to scan `{path}` because {reason}")]
    NonLocal { path: PathBuf, reason: String },
    #[error("filesystem locality checks are unsupported on this platform for `{path}`")]
    UnsupportedPlatform { path: PathBuf },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinuxFilesystemLocality {
    Local,
    NonLocal,
    Unknown,
}

#[derive(Debug)]
struct LinuxMountEntry {
    mount_point: PathBuf,
    filesystem: String,
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_SYMLINK_EXPANSIONS: usize = 64;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_PATH_COMPONENTS: usize = 4_096;
#[cfg(unix)]
const MAX_UNIX_PATH_BYTES: usize = 1024 * 1024;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_SYMLINK_TARGET_BYTES_TOTAL: usize = MAX_UNIX_PATH_BYTES;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_COMPONENT_VISITS: usize = 16_384;
#[cfg(unix)]
const MAX_UNIX_TREE_ENTRIES: usize = 1_000_000;
#[cfg(unix)]
const MAX_UNIX_TREE_DEPTH: usize = 256;
#[cfg(unix)]
const MAX_UNIX_TREE_FRONTIER: usize = 65_536;
#[cfg(unix)]
const MAX_UNIX_TREE_FRONTIER_PATH_BYTES: usize = 64 * 1_048_576;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_DIRECTORY_ENTRIES_PER_LOOKUP: usize = 1_000_000;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_DIRECTORY_NAME_BYTES_PER_LOOKUP: usize = 64 * 1024 * 1024;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_RECONCILIATION_ENTRIES_TOTAL: usize = 1_000_000;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_RECONCILIATION_NAME_BYTES_TOTAL: usize = 64 * 1024 * 1024;
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
const MAX_UNIX_RECONCILIATION_MOUNT_RECORDS_TOTAL: usize = 65_536;

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
#[derive(Debug)]
struct UnixReconciliationBudget {
    directory_entries: usize,
    directory_name_bytes: usize,
    mount_records: usize,
    deadline: Option<std::time::Instant>,
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
impl UnixReconciliationBudget {
    fn new(deadline: Option<std::time::Instant>) -> Self {
        Self {
            directory_entries: 0,
            directory_name_bytes: 0,
            mount_records: 0,
            deadline,
        }
    }

    fn check_deadline(&self) -> std::io::Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Unix component reconciliation exceeded its operation deadline",
            ));
        }
        Ok(())
    }

    fn observe_mount_record(&mut self) -> std::io::Result<()> {
        self.check_deadline()?;
        self.mount_records = self
            .mount_records
            .checked_add(1)
            .ok_or_else(|| invalid_unix_path("Unix mount-reconciliation accounting overflowed"))?;
        if self.mount_records > MAX_UNIX_RECONCILIATION_MOUNT_RECORDS_TOTAL {
            return Err(invalid_unix_path(
                "Unix path exceeds its cumulative mount-reconciliation limit",
            ));
        }
        Ok(())
    }

    fn observe_directory_entry(&mut self, name_bytes: usize) -> std::io::Result<()> {
        self.check_deadline()?;
        self.directory_entries = self.directory_entries.checked_add(1).ok_or_else(|| {
            invalid_unix_path("Unix directory-reconciliation accounting overflowed")
        })?;
        self.directory_name_bytes = self
            .directory_name_bytes
            .checked_add(name_bytes)
            .ok_or_else(|| invalid_unix_path("Unix directory-entry name accounting overflowed"))?;
        if self.directory_entries > MAX_UNIX_RECONCILIATION_ENTRIES_TOTAL
            || self.directory_name_bytes > MAX_UNIX_RECONCILIATION_NAME_BYTES_TOTAL
        {
            return Err(invalid_unix_path(
                "Unix path exceeds its cumulative component-reconciliation limit",
            ));
        }
        Ok(())
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn check_unix_resolution_deadline(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    check_unix_io_deadline(deadline).map_err(|source| LocalFilesystemError::Inspection {
        path: bounded_unix_error_path(path),
        source,
    })
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn check_unix_io_deadline(deadline: Option<std::time::Instant>) -> std::io::Result<()> {
    if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "Unix filesystem inspection exceeded its operation deadline",
        ));
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnixPathResolutionMode {
    Existing,
    NearestExistingAncestor,
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
enum UnixPathComponent {
    CurDir,
    ParentDir,
    Normal(OsString),
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnixPathKind {
    Directory,
    Symlink,
    Other,
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
#[derive(Debug)]
struct BsdMountEntry {
    mount_point: PathBuf,
    filesystem: String,
    local: bool,
}

/// Certifies a root that will be traversed recursively.
///
/// Both the containing mount and every nested mount below the root must be
/// local. This is a point-in-time gate: Unix and BSD combine mount-table checks
/// with a bounded no-follow descendant walk, while Windows performs its bounded
/// walk on a cancellable file-I/O worker. No platform pins the certified tree
/// against later mutation.
pub(super) fn require_local_tree(path: &Path) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    require_local_filesystem_impl(path, true)?;
    #[cfg(unix)]
    require_direct_unix_tree(path, None)?;
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn require_local_tree_until(
    path: &Path,
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    require_local_filesystem_impl_with_deadline(path, true, Some(deadline))?;
    #[cfg(unix)]
    require_direct_unix_tree(path, Some(deadline))?;
    Ok(())
}

/// Certifies the containing and nested mount boundaries without opening any
/// descendant entry. Callers that perform their own bounded, no-follow walk
/// use this to establish locality before the first descendant metadata or
/// directory operation while retaining their narrower traversal scope.
#[cfg(not(windows))]
pub(super) fn require_local_tree_mounts_until(
    path: &Path,
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    require_local_filesystem_impl_with_deadline(path, true, Some(deadline))
}

#[cfg(windows)]
pub(super) fn require_local_tree_until(
    path: &Path,
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    super::windows::require_local_tree_until(path, deadline).map_err(|source| {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(windows)]
pub(super) fn require_local_tree_with_exclusions_until(
    path: &Path,
    exclusions: &[PathBuf],
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    super::windows::require_local_tree_with_exclusions_until(path, exclusions, deadline).map_err(
        |source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        },
    )
}

#[cfg(windows)]
pub(super) fn require_local_tree_with_scope_until(
    path: &Path,
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    super::windows::require_local_tree_with_scope_until(path, exclusions, inclusions, deadline)
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })
}

/// Certifies only the filesystem containing an existing path.
///
/// Use this for direct file inspection and direct-child lookup. Unrelated
/// descendant mounts are intentionally ignored because the caller will not
/// traverse them.
pub(super) fn require_local_containing_path(path: &Path) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    require_local_filesystem_impl(path, false)
}

#[cfg(not(windows))]
pub(super) fn require_local_containing_path_until(
    path: &Path,
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_existing_path(path)?;
    require_local_filesystem_impl_with_deadline(path, false, Some(deadline))
}

/// Certifies the containing filesystem before a path is created.
///
/// Only the nearest existing ancestor is inspected. The caller must certify
/// the completed path again after creation so mount changes fail closed.
pub(super) fn require_local_filesystem_for_creation(
    path: &Path,
) -> Result<(), LocalFilesystemError> {
    validate_creation_path(path)?;
    require_local_filesystem_for_creation_impl(path)
}

#[cfg(not(windows))]
pub(super) fn require_local_filesystem_for_creation_until(
    path: &Path,
    deadline: std::time::Instant,
) -> Result<(), LocalFilesystemError> {
    validate_creation_path(path)?;
    require_local_filesystem_for_creation_impl_with_deadline(path, Some(deadline))
}

fn validate_creation_path(path: &Path) -> Result<(), LocalFilesystemError> {
    validate_nonempty_path(path, "creation path")?;
    validate_path_size(path)?;
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "creation path may not contain parent components",
            ),
        });
    }
    Ok(())
}

fn validate_existing_path(path: &Path) -> Result<(), LocalFilesystemError> {
    validate_nonempty_path(path, "existing path")?;
    validate_path_size(path)
}

pub(super) fn validate_path_size(path: &Path) -> Result<(), LocalFilesystemError> {
    #[cfg(unix)]
    {
        validate_unix_path_bytes(path).map_err(|source| LocalFilesystemError::Inspection {
            path: bounded_unix_error_path(path),
            source,
        })
    }
    #[cfg(windows)]
    {
        super::windows::validate_windows_path_units(path).map_err(|source| {
            LocalFilesystemError::Inspection {
                path: PathBuf::from("<oversized Windows path>"),
                source,
            }
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(())
    }
}

fn validate_nonempty_path(path: &Path, label: &'static str) -> Result<(), LocalFilesystemError> {
    if path.as_os_str().is_empty() {
        return Err(LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{label} must not be empty"),
            ),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn require_direct_unix_tree(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    fn inspection(path: &Path, source: std::io::Error) -> LocalFilesystemError {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        }
    }

    fn check_deadline(
        path: &Path,
        deadline: Option<std::time::Instant>,
    ) -> Result<(), LocalFilesystemError> {
        if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
            return Err(inspection(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Unix tree certification exceeded its operation deadline",
                ),
            ));
        }
        Ok(())
    }

    check_deadline(path, deadline)?;
    let root = std::fs::canonicalize(path).map_err(|source| inspection(path, source))?;
    check_deadline(&root, deadline)?;
    let metadata = std::fs::symlink_metadata(&root).map_err(|source| inspection(&root, source))?;
    check_deadline(&root, deadline)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(inspection(
            &root,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "a recursively certified Unix root must be a direct directory",
            ),
        ));
    }
    let root_path_bytes = root.as_os_str().as_bytes().len();
    if root_path_bytes > MAX_UNIX_TREE_FRONTIER_PATH_BYTES {
        return Err(unix_tree_limit(
            &root,
            "pending-directory path bytes",
            MAX_UNIX_TREE_FRONTIER_PATH_BYTES,
        ));
    }
    let mut frontier = vec![(root.clone(), 0_usize, root_path_bytes)];
    let mut frontier_path_bytes = root_path_bytes;
    let mut entries = 0_usize;
    while let Some((directory, depth, directory_path_bytes)) = frontier.pop() {
        frontier_path_bytes = frontier_path_bytes
            .checked_sub(directory_path_bytes)
            .ok_or_else(|| {
                inspection(
                    &root,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Unix tree frontier path accounting underflowed",
                    ),
                )
            })?;
        check_deadline(&directory, deadline)?;
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|source| inspection(&directory, source))?;
        check_deadline(&directory, deadline)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(inspection(
                &directory,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "a recursively inspected Unix directory changed type",
                ),
            ));
        }
        let mut children =
            std::fs::read_dir(&directory).map_err(|source| inspection(&directory, source))?;
        check_deadline(&directory, deadline)?;
        loop {
            check_deadline(&directory, deadline)?;
            let Some(child) = children.next() else {
                break;
            };
            check_deadline(&directory, deadline)?;
            let child = child.map_err(|source| inspection(&directory, source))?;
            let child_path = child.path();
            entries = entries
                .checked_add(1)
                .ok_or_else(|| unix_tree_limit(&root, "entry count", MAX_UNIX_TREE_ENTRIES))?;
            if entries > MAX_UNIX_TREE_ENTRIES {
                return Err(unix_tree_limit(&root, "entry count", MAX_UNIX_TREE_ENTRIES));
            }
            check_deadline(&child_path, deadline)?;
            let metadata = std::fs::symlink_metadata(&child_path)
                .map_err(|source| inspection(&child_path, source))?;
            check_deadline(&child_path, deadline)?;
            if metadata.file_type().is_symlink() {
                return Err(inspection(
                    &child_path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "a recursively certified Unix tree must not contain symbolic links",
                    ),
                ));
            }
            if metadata.is_file() {
                continue;
            }
            if !metadata.is_dir() {
                return Err(inspection(
                    &child_path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "a recursively certified Unix tree must contain only files and directories",
                    ),
                ));
            }
            let child_depth = depth
                .checked_add(1)
                .ok_or_else(|| unix_tree_limit(&root, "directory depth", MAX_UNIX_TREE_DEPTH))?;
            if child_depth > MAX_UNIX_TREE_DEPTH {
                return Err(unix_tree_limit(
                    &root,
                    "directory depth",
                    MAX_UNIX_TREE_DEPTH,
                ));
            }
            if frontier.len() >= MAX_UNIX_TREE_FRONTIER {
                return Err(unix_tree_limit(
                    &root,
                    "pending-directory frontier",
                    MAX_UNIX_TREE_FRONTIER,
                ));
            }
            let child_path_bytes = child_path.as_os_str().as_bytes().len();
            let next_frontier_path_bytes = frontier_path_bytes
                .checked_add(child_path_bytes)
                .ok_or_else(|| {
                    unix_tree_limit(
                        &root,
                        "pending-directory path bytes",
                        MAX_UNIX_TREE_FRONTIER_PATH_BYTES,
                    )
                })?;
            if next_frontier_path_bytes > MAX_UNIX_TREE_FRONTIER_PATH_BYTES {
                return Err(unix_tree_limit(
                    &root,
                    "pending-directory path bytes",
                    MAX_UNIX_TREE_FRONTIER_PATH_BYTES,
                ));
            }
            frontier.try_reserve(1).map_err(|error| {
                inspection(
                    &root,
                    std::io::Error::new(
                        std::io::ErrorKind::OutOfMemory,
                        format!("could not allocate the Unix tree frontier: {error}"),
                    ),
                )
            })?;
            frontier.push((child_path, child_depth, child_path_bytes));
            frontier_path_bytes = next_frontier_path_bytes;
        }
    }
    check_deadline(&root, deadline)
}

#[cfg(unix)]
fn unix_tree_limit(path: &Path, dimension: &str, limit: usize) -> LocalFilesystemError {
    LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unix tree certification exceeded its {dimension} limit ({limit})"),
        ),
    }
}

#[cfg(windows)]
fn require_local_filesystem_for_creation_impl(path: &Path) -> Result<(), LocalFilesystemError> {
    let absolute = super::windows::require_local_creation_volume(path).map_err(|source| {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let mut ancestors = absolute.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    let mut nearest_existing = None;
    for (index, ancestor) in ancestors.iter().enumerate() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if super::windows::is_reparse_point(&metadata) {
                    return Err(LocalFilesystemError::NonLocal {
                        path: (*ancestor).to_path_buf(),
                        reason: "a creation-path ancestor is a Windows reparse point".to_string(),
                    });
                }
                if index + 1 < ancestors.len() && !metadata.is_dir() {
                    return Err(LocalFilesystemError::Inspection {
                        path: (*ancestor).to_path_buf(),
                        source: std::io::Error::new(
                            std::io::ErrorKind::NotADirectory,
                            "a creation-path ancestor is not a directory",
                        ),
                    });
                }
                nearest_existing = Some(*ancestor);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(LocalFilesystemError::Inspection {
                    path: (*ancestor).to_path_buf(),
                    source,
                });
            }
        }
    }
    if let Some(ancestor) = nearest_existing {
        return require_local_containing_path(ancestor);
    }
    Err(LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no existing ancestor contains the candidate path",
        ),
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    windows
)))]
fn require_local_filesystem_for_creation_impl(path: &Path) -> Result<(), LocalFilesystemError> {
    Err(LocalFilesystemError::UnsupportedPlatform {
        path: path.to_path_buf(),
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    windows
)))]
fn require_local_filesystem_for_creation_impl_with_deadline(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return Err(LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "filesystem locality certification exceeded its operation deadline",
            ),
        });
    }
    require_local_filesystem_for_creation_impl(path)
}

#[cfg(target_os = "linux")]
fn require_local_filesystem_impl(
    path: &Path,
    inspect_nested_mounts: bool,
) -> Result<(), LocalFilesystemError> {
    require_local_filesystem_impl_with_deadline(path, inspect_nested_mounts, None)
}

#[cfg(target_os = "linux")]
fn require_local_filesystem_impl_with_deadline(
    path: &Path,
    inspect_nested_mounts: bool,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    let mut reconciliation_budget = UnixReconciliationBudget::new(deadline);
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let mountinfo = read_mountinfo(path, deadline)?;
    let mounts =
        linux_mount_entries(&mountinfo).map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let resolved = resolve_existing_unix_path(
        path,
        deadline,
        |candidate| certify_linux_containing_mount(&mounts, candidate),
        |parent, component| {
            exact_unix_component_for_mounts(
                parent,
                component,
                mounts.iter().map(|mount| mount.mount_point.as_path()),
                &mut reconciliation_budget,
            )
        },
    )?;
    if inspect_nested_mounts {
        certify_linux_nested_mounts(&mounts, &resolved)?;
    }
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_local_filesystem_for_creation_impl(path: &Path) -> Result<(), LocalFilesystemError> {
    require_local_filesystem_for_creation_impl_with_deadline(path, None)
}

#[cfg(target_os = "linux")]
fn require_local_filesystem_for_creation_impl_with_deadline(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    let mut reconciliation_budget = UnixReconciliationBudget::new(deadline);
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let mountinfo = read_mountinfo(path, deadline)?;
    let mounts =
        linux_mount_entries(&mountinfo).map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    resolve_nearest_existing_unix_ancestor(
        path,
        deadline,
        |candidate| certify_linux_containing_mount(&mounts, candidate),
        |parent, component| {
            exact_unix_component_for_mounts(
                parent,
                component,
                mounts.iter().map(|mount| mount.mount_point.as_path()),
                &mut reconciliation_budget,
            )
        },
    )?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn certify_linux_containing_mount(
    mounts: &[LinuxMountEntry],
    path: &Path,
) -> Result<(), LocalFilesystemError> {
    let filesystem = filesystem_type_for_mounts(mounts, path).ok_or_else(|| {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: invalid_mountinfo("no mount contains the inspected path"),
        }
    })?;
    match classify_linux_filesystem_type(filesystem) {
        LinuxFilesystemLocality::Local => Ok(()),
        LinuxFilesystemLocality::NonLocal => Err(LocalFilesystemError::NonLocal {
            path: path.to_path_buf(),
            reason: format!("filesystem type `{filesystem}` is non-local"),
        }),
        LinuxFilesystemLocality::Unknown => Err(LocalFilesystemError::NonLocal {
            path: path.to_path_buf(),
            reason: format!("filesystem type `{filesystem}` is not in the local allowlist"),
        }),
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn certify_linux_nested_mounts(
    mounts: &[LinuxMountEntry],
    root: &Path,
) -> Result<(), LocalFilesystemError> {
    for mount in mounts
        .iter()
        .filter(|mount| mount.mount_point.starts_with(root))
    {
        match classify_linux_filesystem_type(&mount.filesystem) {
            LinuxFilesystemLocality::Local => {}
            LinuxFilesystemLocality::NonLocal => {
                return Err(LocalFilesystemError::NonLocal {
                    path: mount.mount_point.clone(),
                    reason: format!("nested filesystem type `{}` is non-local", mount.filesystem),
                });
            }
            LinuxFilesystemLocality::Unknown => {
                return Err(LocalFilesystemError::NonLocal {
                    path: mount.mount_point.clone(),
                    reason: format!(
                        "nested filesystem type `{}` is not in the local allowlist",
                        mount.filesystem
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_mountinfo(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<Vec<u8>, LocalFilesystemError> {
    use std::io::Read;

    const MAX_MOUNTINFO_BYTES: u64 = 16 * 1024 * 1024;

    check_unix_resolution_deadline(path, deadline)?;
    let file = std::fs::File::open("/proc/self/mountinfo").map_err(|source| {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        }
    })?;
    check_unix_resolution_deadline(path, deadline)?;
    let mut bytes = Vec::new();
    let mut reader = file.take(MAX_MOUNTINFO_BYTES + 1);
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        check_unix_resolution_deadline(path, deadline)?;
        let read = reader.read(&mut buffer);
        check_unix_resolution_deadline(path, deadline)?;
        let read = match read {
            Ok(read) => read,
            Err(source) if source.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(source) => {
                return Err(LocalFilesystemError::Inspection {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        if read == 0 {
            break;
        }
        bytes
            .try_reserve(read)
            .map_err(|error| LocalFilesystemError::Inspection {
                path: path.to_path_buf(),
                source: std::io::Error::other(format!(
                    "could not allocate a bounded Linux mount-table snapshot: {error}"
                )),
            })?;
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.len() as u64 > MAX_MOUNTINFO_BYTES {
        return Err(LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Linux mount table exceeds the inspection limit",
            ),
        });
    }
    Ok(bytes)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn filesystem_type_for_path(mountinfo: &[u8], path: &Path) -> std::io::Result<String> {
    let mounts = linux_mount_entries(mountinfo)?;
    filesystem_type_for_mounts(&mounts, path)
        .map(str::to_owned)
        .ok_or_else(|| invalid_mountinfo("no mount contains the inspected path"))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn linux_mount_entries(mountinfo: &[u8]) -> std::io::Result<Vec<LinuxMountEntry>> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    const MAX_MOUNT_ENTRIES: usize = 65_536;

    let mut mounts = Vec::new();
    for line in mountinfo
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let separator = line
            .windows(3)
            .position(|window| window == b" - ")
            .ok_or_else(|| invalid_mountinfo("mount entry has no field separator"))?;
        let mount_point = line[..separator]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .nth(4)
            .ok_or_else(|| invalid_mountinfo("mount entry has no mount point"))?;
        let filesystem = line[separator + 3..]
            .split(|byte| byte.is_ascii_whitespace())
            .find(|field| !field.is_empty())
            .ok_or_else(|| invalid_mountinfo("mount entry has no filesystem type"))?;
        let filesystem = std::str::from_utf8(filesystem)
            .map_err(|_| invalid_mountinfo("filesystem type is not UTF-8"))?;
        mounts.push(LinuxMountEntry {
            mount_point: PathBuf::from(OsString::from_vec(decode_mountinfo_field(mount_point)?)),
            filesystem: filesystem.to_owned(),
        });
        if mounts.len() > MAX_MOUNT_ENTRIES {
            return Err(invalid_mountinfo("mount table exceeds its entry limit"));
        }
    }
    Ok(mounts)
}

fn filesystem_type_for_mounts<'a>(mounts: &'a [LinuxMountEntry], path: &Path) -> Option<&'a str> {
    let deepest = mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.mount_point))
        .map(|mount| mount.mount_point.components().count())
        .max()?;
    let mut first = None;
    for mount in mounts.iter().filter(|mount| {
        path.starts_with(&mount.mount_point) && mount.mount_point.components().count() == deepest
    }) {
        first.get_or_insert(mount);
        if classify_linux_filesystem_type(&mount.filesystem) != LinuxFilesystemLocality::Local {
            return Some(mount.filesystem.as_str());
        }
    }
    first.map(|mount| mount.filesystem.as_str())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn decode_mountinfo_field(field: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut decoded = Vec::with_capacity(field.len());
    let mut cursor = 0;
    while cursor < field.len() {
        if field[cursor] != b'\\' {
            decoded.push(field[cursor]);
            cursor += 1;
            continue;
        }

        let Some(octal) = field.get(cursor + 1..cursor + 4) else {
            return Err(invalid_mountinfo("mount point has a truncated escape"));
        };
        if !octal.iter().all(u8::is_ascii_digit) || octal.iter().any(|digit| *digit > b'7') {
            return Err(invalid_mountinfo("mount point has an invalid escape"));
        }
        let value = u16::from(octal[0] - b'0') * 64
            + u16::from(octal[1] - b'0') * 8
            + u16::from(octal[2] - b'0');
        decoded.push(
            u8::try_from(value)
                .map_err(|_| invalid_mountinfo("mount point escape exceeds one byte"))?,
        );
        cursor += 4;
    }
    Ok(decoded)
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn invalid_mountinfo(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn resolve_existing_unix_path(
    path: &Path,
    deadline: Option<std::time::Instant>,
    certify: impl FnMut(&Path) -> Result<(), LocalFilesystemError>,
    exact_component: impl FnMut(&Path, &std::ffi::OsStr) -> std::io::Result<bool>,
) -> Result<PathBuf, LocalFilesystemError> {
    resolve_unix_path_with(
        path,
        UnixPathResolutionMode::Existing,
        deadline,
        certify,
        exact_component,
        inspect_unix_path_kind,
        |candidate| std::fs::read_link(candidate),
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn resolve_nearest_existing_unix_ancestor(
    path: &Path,
    deadline: Option<std::time::Instant>,
    certify: impl FnMut(&Path) -> Result<(), LocalFilesystemError>,
    exact_component: impl FnMut(&Path, &std::ffi::OsStr) -> std::io::Result<bool>,
) -> Result<PathBuf, LocalFilesystemError> {
    resolve_unix_path_with(
        path,
        UnixPathResolutionMode::NearestExistingAncestor,
        deadline,
        certify,
        exact_component,
        inspect_unix_path_kind,
        |candidate| std::fs::read_link(candidate),
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn resolve_unix_path_with(
    path: &Path,
    mode: UnixPathResolutionMode,
    deadline: Option<std::time::Instant>,
    mut certify: impl FnMut(&Path) -> Result<(), LocalFilesystemError>,
    mut exact_component: impl FnMut(&Path, &std::ffi::OsStr) -> std::io::Result<bool>,
    mut inspect_path: impl FnMut(&Path) -> std::io::Result<UnixPathKind>,
    mut read_link: impl FnMut(&Path) -> std::io::Result<PathBuf>,
) -> Result<PathBuf, LocalFilesystemError> {
    use std::os::unix::ffi::OsStrExt;

    check_unix_resolution_deadline(path, deadline)?;
    let absolute = absolute_unix_path(path).map_err(|source| LocalFilesystemError::Inspection {
        path: bounded_unix_error_path(path),
        source,
    })?;
    check_unix_resolution_deadline(path, deadline)?;

    // Parent components stay queued until every preceding symbolic link has
    // been resolved, matching the kernel's path-walk order.
    let mut pending = unix_resolution_components(&absolute).map_err(|source| {
        LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let mut resolved = PathBuf::from("/");
    validate_unix_resolution_bytes(&resolved, pending.iter()).map_err(|source| {
        LocalFilesystemError::Inspection {
            path: bounded_unix_error_path(path),
            source,
        }
    })?;
    let mut resolved_is_directory = true;
    let mut symlink_expansions = 0_usize;
    let mut symlink_target_bytes = 0_usize;
    let mut component_visits = 0_usize;

    while let Some(component) = pending.pop_front() {
        check_unix_resolution_deadline(&resolved, deadline)?;
        component_visits = component_visits.saturating_add(1);
        if component_visits > MAX_UNIX_COMPONENT_VISITS {
            return Err(unix_resolution_limit(path, "component visits"));
        }
        match component {
            UnixPathComponent::CurDir => {
                if !resolved_is_directory {
                    return Err(unix_not_a_directory(&resolved));
                }
                continue;
            }
            UnixPathComponent::ParentDir => {
                if !resolved_is_directory {
                    return Err(unix_not_a_directory(&resolved));
                }
                resolved.pop();
                resolved_is_directory = true;
                continue;
            }
            UnixPathComponent::Normal(component) => {
                if !resolved_is_directory {
                    return Err(unix_not_a_directory(&resolved));
                }

                check_unix_resolution_deadline(&resolved, deadline)?;
                certify(&resolved)?;
                check_unix_resolution_deadline(&resolved, deadline)?;
                let candidate = resolved.join(&component);
                check_unix_resolution_deadline(&candidate, deadline)?;
                let component_is_exact = exact_component(&resolved, &component);
                check_unix_resolution_deadline(&candidate, deadline)?;
                let component_is_exact =
                    component_is_exact.map_err(|source| LocalFilesystemError::Inspection {
                        path: candidate.clone(),
                        source,
                    })?;
                if !component_is_exact {
                    check_unix_resolution_deadline(&candidate, deadline)?;
                    let candidate_kind = inspect_path(&candidate);
                    check_unix_resolution_deadline(&candidate, deadline)?;
                    match candidate_kind {
                        Err(source)
                            if source.kind() == std::io::ErrorKind::NotFound
                                && mode == UnixPathResolutionMode::NearestExistingAncestor =>
                        {
                            if pending
                                .iter()
                                .any(|component| matches!(component, UnixPathComponent::ParentDir))
                            {
                                return Err(LocalFilesystemError::Inspection {
                                    path: candidate,
                                    source: invalid_unix_path(
                                        "creation path may not traverse a parent after its first missing component",
                                    ),
                                });
                            }
                            check_unix_resolution_deadline(&resolved, deadline)?;
                            certify(&resolved)?;
                            check_unix_resolution_deadline(&resolved, deadline)?;
                            return Ok(resolved);
                        }
                        Err(source) => {
                            return Err(LocalFilesystemError::Inspection {
                                path: candidate,
                                source,
                            });
                        }
                        Ok(_) => return Err(non_exact_unix_component(&candidate)),
                    }
                }

                check_unix_resolution_deadline(&candidate, deadline)?;
                certify(&candidate)?;
                check_unix_resolution_deadline(&candidate, deadline)?;
                let candidate_kind = inspect_path(&candidate);
                check_unix_resolution_deadline(&candidate, deadline)?;
                let candidate_kind = match candidate_kind {
                    Ok(candidate_kind) => candidate_kind,
                    Err(source)
                        if source.kind() == std::io::ErrorKind::NotFound
                            && mode == UnixPathResolutionMode::NearestExistingAncestor =>
                    {
                        if pending
                            .iter()
                            .any(|component| matches!(component, UnixPathComponent::ParentDir))
                        {
                            return Err(LocalFilesystemError::Inspection {
                                path: candidate,
                                source: invalid_unix_path(
                                    "creation path may not traverse a parent after its first missing component",
                                ),
                            });
                        }
                        check_unix_resolution_deadline(&resolved, deadline)?;
                        certify(&resolved)?;
                        check_unix_resolution_deadline(&resolved, deadline)?;
                        return Ok(resolved);
                    }
                    Err(source) => {
                        return Err(LocalFilesystemError::Inspection {
                            path: candidate,
                            source,
                        });
                    }
                };
                if candidate_kind != UnixPathKind::Symlink {
                    resolved = candidate;
                    resolved_is_directory = candidate_kind == UnixPathKind::Directory;
                    continue;
                }

                symlink_expansions = symlink_expansions.saturating_add(1);
                if symlink_expansions > MAX_UNIX_SYMLINK_EXPANSIONS {
                    return Err(unix_resolution_limit(path, "symbolic-link expansions"));
                }
                check_unix_resolution_deadline(&candidate, deadline)?;
                let target = read_link(&candidate);
                check_unix_resolution_deadline(&candidate, deadline)?;
                let target = target.map_err(|source| LocalFilesystemError::Inspection {
                    path: candidate.clone(),
                    source,
                })?;
                validate_unix_path_bytes(&target).map_err(|source| {
                    LocalFilesystemError::Inspection {
                        path: candidate.clone(),
                        source,
                    }
                })?;
                symlink_target_bytes = symlink_target_bytes
                    .checked_add(target.as_os_str().as_bytes().len())
                    .ok_or_else(|| LocalFilesystemError::Inspection {
                        path: candidate.clone(),
                        source: invalid_unix_path("Unix symlink-target accounting overflowed"),
                    })?;
                if symlink_target_bytes > MAX_UNIX_SYMLINK_TARGET_BYTES_TOTAL {
                    return Err(LocalFilesystemError::Inspection {
                        path: candidate,
                        source: invalid_unix_path(
                            "Unix path exceeds its cumulative symlink-target byte limit",
                        ),
                    });
                }
                let target_is_absolute = target.is_absolute();
                let mut redirected = unix_resolution_components(&target).map_err(|source| {
                    LocalFilesystemError::Inspection {
                        path: candidate.clone(),
                        source,
                    }
                })?;
                let resolved_depth = if target_is_absolute {
                    0
                } else {
                    resolved
                        .components()
                        .filter(|component| matches!(component, std::path::Component::Normal(_)))
                        .count()
                };
                let available_components = MAX_UNIX_PATH_COMPONENTS
                    .saturating_sub(resolved_depth)
                    .saturating_sub(pending.len());
                if redirected.len() > available_components {
                    return Err(LocalFilesystemError::Inspection {
                        path: candidate,
                        source: invalid_unix_path("Unix path exceeds its component limit"),
                    });
                }
                let redirected_base = if target_is_absolute {
                    Path::new("/")
                } else {
                    resolved.as_path()
                };
                validate_unix_resolution_bytes(
                    redirected_base,
                    redirected.iter().chain(pending.iter()),
                )
                .map_err(|source| LocalFilesystemError::Inspection {
                    path: candidate.clone(),
                    source,
                })?;
                redirected.append(&mut pending);
                pending = redirected;
                if target_is_absolute {
                    resolved = PathBuf::from("/");
                    resolved_is_directory = true;
                }
            }
        }
    }
    check_unix_resolution_deadline(&resolved, deadline)?;
    certify(&resolved)?;
    check_unix_resolution_deadline(&resolved, deadline)?;
    Ok(resolved)
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn inspect_unix_path_kind(path: &Path) -> std::io::Result<UnixPathKind> {
    let file_type = std::fs::symlink_metadata(path)?.file_type();
    if file_type.is_symlink() {
        Ok(UnixPathKind::Symlink)
    } else if file_type.is_dir() {
        Ok(UnixPathKind::Directory)
    } else {
        Ok(UnixPathKind::Other)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn exact_unix_component_for_mounts<'a>(
    parent: &Path,
    component: &std::ffi::OsStr,
    mount_points: impl IntoIterator<Item = &'a Path>,
    budget: &mut UnixReconciliationBudget,
) -> std::io::Result<bool> {
    use std::os::unix::ffi::OsStrExt;

    let mut mount_frontier = Vec::<(OsString, bool)>::new();
    for mount_point in mount_points {
        budget.observe_mount_record()?;
        let Ok(relative) = mount_point.strip_prefix(parent) else {
            continue;
        };
        let mut components = relative.components();
        let Some(first) = components.next() else {
            continue;
        };
        let std::path::Component::Normal(name) = first else {
            return Err(invalid_unix_path(
                "mount table path was not lexically normalized",
            ));
        };
        let direct = components.next().is_none();
        mount_frontier.try_reserve(1).map_err(|error| {
            std::io::Error::other(format!(
                "could not allocate a Unix mount-spelling frontier: {error}"
            ))
        })?;
        mount_frontier.push((name.to_os_string(), direct));
    }
    mount_frontier.sort_unstable_by(|left, right| left.0.cmp(&right.0));

    let mut seen_mount_spelling = Vec::new();
    seen_mount_spelling
        .try_reserve_exact(mount_frontier.len())
        .map_err(|error| {
            std::io::Error::other(format!(
                "could not allocate Unix mount-spelling state: {error}"
            ))
        })?;
    seen_mount_spelling.resize(mount_frontier.len(), false);

    let mut exact = false;
    let mut entry_count = 0_usize;
    let mut name_bytes = 0_usize;
    budget.check_deadline()?;
    let mut entries = std::fs::read_dir(parent)?;
    budget.check_deadline()?;
    loop {
        budget.check_deadline()?;
        let entry = entries.next();
        budget.check_deadline()?;
        let Some(entry) = entry else {
            break;
        };
        let entry = entry?;
        budget.check_deadline()?;
        entry_count = entry_count.saturating_add(1);
        if entry_count > MAX_UNIX_DIRECTORY_ENTRIES_PER_LOOKUP {
            return Err(invalid_unix_path(
                "directory exceeds the Unix component-reconciliation entry limit",
            ));
        }
        let name = entry.file_name();
        budget.observe_directory_entry(name.as_bytes().len())?;
        name_bytes = name_bytes
            .checked_add(name.as_bytes().len())
            .ok_or_else(|| invalid_unix_path("Unix directory-entry name accounting overflowed"))?;
        if name_bytes > MAX_UNIX_DIRECTORY_NAME_BYTES_PER_LOOKUP {
            return Err(invalid_unix_path(
                "directory exceeds the Unix component-reconciliation byte limit",
            ));
        }
        exact |= name == component;

        let start = mount_frontier.partition_point(|(mount_name, _)| {
            mount_name.as_os_str().cmp(name.as_os_str()).is_lt()
        });
        let end = mount_frontier.partition_point(|(mount_name, _)| {
            !mount_name.as_os_str().cmp(name.as_os_str()).is_gt()
        });
        seen_mount_spelling[start..end].fill(true);
    }
    budget.check_deadline()?;

    if exact {
        if seen_mount_spelling.iter().any(|seen| !seen) {
            return Err(invalid_unix_path(
                "mount table spelling does not match a stored directory entry",
            ));
        }
        Ok(true)
    } else if mount_frontier.iter().any(|(_, direct)| *direct) {
        Err(invalid_unix_path(
            "non-exact Unix component spelling is ambiguous at a mount boundary",
        ))
    } else {
        Ok(false)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn non_exact_unix_component(path: &Path) -> LocalFilesystemError {
    LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Unix path component does not use its stored directory-entry spelling",
        ),
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn unix_not_a_directory(path: &Path) -> LocalFilesystemError {
    LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Unix path component is not a directory",
        ),
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn absolute_unix_path(path: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    if path.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Unix path is empty",
        ));
    }
    validate_unix_path_bytes(path)?;
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let current = std::env::current_dir()?;
        let combined_bytes = current
            .as_os_str()
            .as_bytes()
            .len()
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(path.as_os_str().as_bytes().len()))
            .ok_or_else(|| invalid_unix_path("Unix path byte accounting overflowed"))?;
        if combined_bytes > MAX_UNIX_PATH_BYTES {
            return Err(invalid_unix_path("Unix path exceeds its byte limit"));
        }
        Ok(current.join(path))
    }
}

#[cfg(unix)]
fn validate_unix_path_bytes(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    if path.as_os_str().as_bytes().len() > MAX_UNIX_PATH_BYTES {
        return Err(invalid_unix_path("Unix path exceeds its byte limit"));
    }
    Ok(())
}

#[cfg(unix)]
fn bounded_unix_error_path(path: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;

    if path.as_os_str().as_bytes().len() > MAX_UNIX_PATH_BYTES {
        PathBuf::from("<oversized Unix path>")
    } else {
        path.to_path_buf()
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn unix_resolution_components(path: &Path) -> std::io::Result<VecDeque<UnixPathComponent>> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    validate_unix_path_bytes(path)?;
    let bytes = path.as_os_str().as_bytes();
    let mut components = VecDeque::new();
    for component in bytes.split(|byte| *byte == b'/') {
        match component {
            b"" => continue,
            b"." => components.push_back(UnixPathComponent::CurDir),
            b".." => components.push_back(UnixPathComponent::ParentDir),
            component => components.push_back(UnixPathComponent::Normal(OsString::from_vec(
                component.to_vec(),
            ))),
        }
        if components.len() > MAX_UNIX_PATH_COMPONENTS {
            return Err(invalid_unix_path("Unix path exceeds its component limit"));
        }
    }
    if bytes.len() > 1 && bytes.ends_with(b"/") {
        components.push_back(UnixPathComponent::CurDir);
        if components.len() > MAX_UNIX_PATH_COMPONENTS {
            return Err(invalid_unix_path("Unix path exceeds its component limit"));
        }
    }
    Ok(components)
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn validate_unix_resolution_bytes<'a>(
    base: &Path,
    components: impl IntoIterator<Item = &'a UnixPathComponent>,
) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = base.as_os_str().as_bytes().len();
    for component in components {
        let component_bytes = match component {
            UnixPathComponent::CurDir => 1,
            UnixPathComponent::ParentDir => 2,
            UnixPathComponent::Normal(component) => component.as_os_str().as_bytes().len(),
        };
        bytes = bytes
            .checked_add(1)
            .and_then(|bytes| bytes.checked_add(component_bytes))
            .ok_or_else(|| invalid_unix_path("Unix path byte accounting overflowed"))?;
        if bytes > MAX_UNIX_PATH_BYTES {
            return Err(invalid_unix_path("Unix path exceeds its byte limit"));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn invalid_unix_path(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    all(test, unix)
))]
fn unix_resolution_limit(path: &Path, dimension: &str) -> LocalFilesystemError {
    LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unix path resolution exceeded its {dimension} limit"),
        ),
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn require_local_filesystem_impl(
    path: &Path,
    inspect_nested_mounts: bool,
) -> Result<(), LocalFilesystemError> {
    require_local_filesystem_impl_with_deadline(path, inspect_nested_mounts, None)
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn require_local_filesystem_impl_with_deadline(
    path: &Path,
    inspect_nested_mounts: bool,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    let mut reconciliation_budget = UnixReconciliationBudget::new(deadline);
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let mounts = bsd_mount_entries(path, deadline)?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let resolved = resolve_existing_unix_path(
        path,
        deadline,
        |candidate| certify_bsd_mounts(candidate, &mounts, false),
        |parent, component| {
            exact_unix_component_for_mounts(
                parent,
                component,
                mounts.iter().map(|mount| mount.mount_point.as_path()),
                &mut reconciliation_budget,
            )
        },
    )?;
    certify_bsd_mounts(&resolved, &mounts, inspect_nested_mounts)?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn require_local_filesystem_for_creation_impl(path: &Path) -> Result<(), LocalFilesystemError> {
    require_local_filesystem_for_creation_impl_with_deadline(path, None)
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn require_local_filesystem_for_creation_impl_with_deadline(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    let mut reconciliation_budget = UnixReconciliationBudget::new(deadline);
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    let mounts = bsd_mount_entries(path, deadline)?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    resolve_nearest_existing_unix_ancestor(
        path,
        deadline,
        |candidate| certify_bsd_mounts(candidate, &mounts, false),
        |parent, component| {
            exact_unix_component_for_mounts(
                parent,
                component,
                mounts.iter().map(|mount| mount.mount_point.as_path()),
                &mut reconciliation_budget,
            )
        },
    )?;
    reconciliation_budget
        .check_deadline()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const MAX_BSD_MOUNT_ENTRIES: usize = 65_536;

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd"
))]
type BsdMountStatistic = libc::statfs;

#[cfg(target_os = "netbsd")]
type BsdMountStatistic = libc::statvfs;

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn bsd_mount_entries(
    path: &Path,
    deadline: Option<std::time::Instant>,
) -> Result<Vec<BsdMountEntry>, LocalFilesystemError> {
    let statistics = caller_owned_bsd_mount_snapshot(
        bsd_mount_count,
        |capacity| read_bsd_mount_batch(capacity, deadline),
        deadline,
    )
    .map_err(|source| LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source,
    })?;
    check_unix_resolution_deadline(path, deadline)?;
    statistics
        .iter()
        .map(bsd_mount_entry_from_statistic)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|source| LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd"
))]
fn bsd_mount_entry_from_statistic(mount: &libc::statfs) -> std::io::Result<BsdMountEntry> {
    bsd_mount_entry(
        &mount.f_mntonname,
        &mount.f_fstypename,
        (mount.f_flags as u128) & (libc::MNT_LOCAL as u128) != 0,
    )
}

#[cfg(target_os = "netbsd")]
fn bsd_mount_entry_from_statistic(mount: &libc::statvfs) -> std::io::Result<BsdMountEntry> {
    bsd_mount_entry(
        &mount.f_mntonname,
        &mount.f_fstypename,
        (mount.f_flag as u128) & (libc::MNT_LOCAL as u128) != 0,
    )
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn caller_owned_bsd_mount_snapshot<T>(
    mut count_mounts: impl FnMut() -> std::io::Result<usize>,
    mut read_mounts: impl FnMut(usize) -> std::io::Result<Vec<T>>,
    deadline: Option<std::time::Instant>,
) -> std::io::Result<Vec<T>> {
    check_unix_io_deadline(deadline)?;
    let initial_count = count_mounts()?;
    check_unix_io_deadline(deadline)?;
    validate_bsd_mount_count(initial_count)?;
    let sentinel_capacity = MAX_BSD_MOUNT_ENTRIES.saturating_add(1);
    let mut capacity = initial_count
        .saturating_mul(2)
        .max(16)
        .min(sentinel_capacity);

    loop {
        check_unix_io_deadline(deadline)?;
        let mounts = read_mounts(capacity)?;
        check_unix_io_deadline(deadline)?;
        validate_bsd_mount_count(mounts.len())?;
        if mounts.len() < capacity {
            return Ok(mounts);
        }
        if capacity == sentinel_capacity {
            return Err(invalid_bsd_mount_table(
                "mount table changed beyond its entry limit during inspection",
            ));
        }
        capacity = capacity.saturating_mul(2).min(sentinel_capacity);
    }
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn validate_bsd_mount_count(count: usize) -> std::io::Result<()> {
    if count == 0 {
        return Err(invalid_bsd_mount_table("mount table was empty"));
    }
    if count > MAX_BSD_MOUNT_ENTRIES {
        return Err(invalid_bsd_mount_table(
            "mount table exceeds its entry limit",
        ));
    }
    Ok(())
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
#[allow(unsafe_code)]
fn read_bsd_mount_batch(
    capacity: usize,
    deadline: Option<std::time::Instant>,
) -> std::io::Result<Vec<BsdMountStatistic>> {
    let mut storage = Vec::<std::mem::MaybeUninit<BsdMountStatistic>>::new();
    storage.try_reserve_exact(capacity).map_err(|error| {
        std::io::Error::other(format!(
            "could not allocate a caller-owned BSD mount snapshot: {error}"
        ))
    })?;
    check_unix_io_deadline(deadline)?;
    // SAFETY: `storage` owns writable, correctly aligned space for at least `capacity` entries.
    // The platform query initializes exactly the number of entries it returns, never more than
    // the supplied capacity.
    let count = unsafe { query_bsd_mounts(storage.as_mut_ptr().cast(), capacity)? };
    if count > capacity {
        return Err(invalid_bsd_mount_table(
            "mount query returned more entries than its caller-owned buffer",
        ));
    }
    let allocation_capacity = storage.capacity();
    let pointer = storage.as_mut_ptr().cast::<BsdMountStatistic>();
    std::mem::forget(storage);
    // SAFETY: The successful query initialized the first `count` entries. `MaybeUninit<T>` has the
    // same layout as `T`, and the original allocation capacity is preserved for deallocation.
    Ok(unsafe { Vec::from_raw_parts(pointer, count, allocation_capacity) })
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
#[allow(unsafe_code)]
fn bsd_mount_count() -> std::io::Result<usize> {
    // SAFETY: A null pointer and zero byte length request only the current mount count.
    unsafe { query_bsd_mounts(std::ptr::null_mut(), 0) }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
unsafe fn query_bsd_mounts(buffer: *mut libc::statfs, capacity: usize) -> std::io::Result<usize> {
    let bytes = checked_bsd_mount_buffer_bytes::<libc::statfs>(capacity)?;
    let bytes = libc::c_int::try_from(bytes)
        .map_err(|_| invalid_bsd_mount_table("mount snapshot byte size exceeds c_int"))?;
    // SAFETY: The caller provides either a null pointer with zero bytes or writable storage for the
    // requested number of statfs entries.
    let count = unsafe { libc::getfsstat(buffer, bytes, libc::MNT_NOWAIT) };
    checked_bsd_mount_query_result(count)
}

#[cfg(target_os = "freebsd")]
#[allow(unsafe_code)]
unsafe fn query_bsd_mounts(buffer: *mut libc::statfs, capacity: usize) -> std::io::Result<usize> {
    let bytes = checked_bsd_mount_buffer_bytes::<libc::statfs>(capacity)?;
    let bytes = libc::c_long::try_from(bytes)
        .map_err(|_| invalid_bsd_mount_table("mount snapshot byte size exceeds c_long"))?;
    // SAFETY: The caller provides either a null pointer with zero bytes or writable storage for the
    // requested number of statfs entries.
    let count = unsafe { libc::getfsstat(buffer, bytes, libc::MNT_NOWAIT) };
    checked_bsd_mount_query_result(count)
}

#[cfg(target_os = "dragonfly")]
#[allow(unsafe_code)]
unsafe extern "C" {
    fn getfsstat(
        buffer: *mut libc::statfs,
        buffer_size: libc::c_long,
        flags: libc::c_int,
    ) -> libc::c_int;
}

#[cfg(target_os = "dragonfly")]
#[allow(unsafe_code)]
unsafe fn query_bsd_mounts(buffer: *mut libc::statfs, capacity: usize) -> std::io::Result<usize> {
    const DRAGONFLY_MNT_NOWAIT: libc::c_int = 2;

    let bytes = checked_bsd_mount_buffer_bytes::<libc::statfs>(capacity)?;
    let bytes = libc::c_long::try_from(bytes)
        .map_err(|_| invalid_bsd_mount_table("mount snapshot byte size exceeds c_long"))?;
    // SAFETY: DragonFly declares getfsstat with this signature. The caller provides either a null
    // pointer with zero bytes or writable storage for the requested number of statfs entries.
    let count = unsafe { getfsstat(buffer, bytes, DRAGONFLY_MNT_NOWAIT) };
    checked_bsd_mount_query_result(count)
}

#[cfg(target_os = "openbsd")]
#[allow(unsafe_code)]
unsafe fn query_bsd_mounts(buffer: *mut libc::statfs, capacity: usize) -> std::io::Result<usize> {
    let bytes = checked_bsd_mount_buffer_bytes::<libc::statfs>(capacity)?;
    // SAFETY: The caller provides either a null pointer with zero bytes or writable storage for the
    // requested number of statfs entries.
    let count = unsafe { libc::getfsstat(buffer, bytes, libc::MNT_NOWAIT) };
    checked_bsd_mount_query_result(count)
}

#[cfg(target_os = "netbsd")]
#[allow(unsafe_code)]
unsafe fn query_bsd_mounts(buffer: *mut libc::statvfs, capacity: usize) -> std::io::Result<usize> {
    let bytes = checked_bsd_mount_buffer_bytes::<libc::statvfs>(capacity)?;
    // SAFETY: The caller provides either a null pointer with zero bytes or writable storage for the
    // requested number of statvfs entries.
    let count = unsafe { libc::getvfsstat(buffer, bytes, libc::MNT_NOWAIT) };
    checked_bsd_mount_query_result(count)
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn checked_bsd_mount_buffer_bytes<T>(capacity: usize) -> std::io::Result<usize> {
    capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| invalid_bsd_mount_table("mount snapshot byte size overflowed"))
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn checked_bsd_mount_query_result(count: libc::c_int) -> std::io::Result<usize> {
    if count < 0 {
        return Err(std::io::Error::last_os_error());
    }
    usize::try_from(count).map_err(|_| invalid_bsd_mount_table("mount count was invalid"))
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn bsd_mount_entry(
    mount_point: &[libc::c_char],
    filesystem: &[libc::c_char],
    local: bool,
) -> std::io::Result<BsdMountEntry> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let mount_point = bounded_bsd_c_string(mount_point, "mount point")?;
    let filesystem = bounded_bsd_c_string(filesystem, "filesystem type")?;
    let filesystem = std::str::from_utf8(&filesystem)
        .map_err(|_| invalid_bsd_mount_table("filesystem type is not UTF-8"))?;
    Ok(BsdMountEntry {
        mount_point: PathBuf::from(OsString::from_vec(mount_point)),
        filesystem: filesystem.to_owned(),
        local,
    })
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn bounded_bsd_c_string(
    field: &[libc::c_char],
    field_name: &'static str,
) -> std::io::Result<Vec<u8>> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid_bsd_mount_table("mount table field was not NUL-terminated"))?;
    if end == 0 {
        return Err(invalid_bsd_mount_table(field_name));
    }
    if end == field.len().saturating_sub(1) {
        return Err(invalid_bsd_mount_table(
            "mount table field may have been truncated",
        ));
    }
    Ok(field[..end].iter().map(|byte| *byte as u8).collect())
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn certify_bsd_mounts(
    canonical_path: &Path,
    mounts: &[BsdMountEntry],
    inspect_nested_mounts: bool,
) -> Result<(), LocalFilesystemError> {
    let deepest = mounts
        .iter()
        .filter(|mount| canonical_path.starts_with(&mount.mount_point))
        .map(|mount| mount.mount_point.components().count())
        .max()
        .ok_or_else(|| LocalFilesystemError::Inspection {
            path: canonical_path.to_path_buf(),
            source: invalid_bsd_mount_table("no mount contains the inspected path"),
        })?;
    let containing_mount = mounts
        .iter()
        .filter(|mount| {
            canonical_path.starts_with(&mount.mount_point)
                && mount.mount_point.components().count() == deepest
        })
        .find(|mount| !mount.local)
        .or_else(|| {
            mounts.iter().find(|mount| {
                canonical_path.starts_with(&mount.mount_point)
                    && mount.mount_point.components().count() == deepest
            })
        })
        .expect("the deepest containing mount was established above");
    if !containing_mount.local {
        return Err(LocalFilesystemError::NonLocal {
            path: canonical_path.to_path_buf(),
            reason: format!(
                "filesystem type `{}` was not marked local by the operating system",
                containing_mount.filesystem
            ),
        });
    }
    if inspect_nested_mounts
        && let Some(mount) = mounts
            .iter()
            .find(|mount| mount.mount_point.starts_with(canonical_path) && !mount.local)
    {
        return Err(LocalFilesystemError::NonLocal {
            path: mount.mount_point.clone(),
            reason: format!(
                "nested filesystem type `{}` was not marked local by the operating system",
                mount.filesystem
            ),
        });
    }
    Ok(())
}

#[cfg(any(
    all(test, unix),
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn invalid_bsd_mount_table(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

#[cfg(windows)]
fn require_local_filesystem_impl(
    path: &Path,
    inspect_nested_mounts: bool,
) -> Result<(), LocalFilesystemError> {
    let result = if inspect_nested_mounts {
        super::windows::require_local_tree(path)
    } else {
        super::windows::require_local_fixed_volume(path)
    };
    result.map_err(|source| LocalFilesystemError::Inspection {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    windows
)))]
fn require_local_filesystem_impl(
    path: &Path,
    _inspect_nested_mounts: bool,
) -> Result<(), LocalFilesystemError> {
    Err(LocalFilesystemError::UnsupportedPlatform {
        path: path.to_path_buf(),
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    windows
)))]
fn require_local_filesystem_impl_with_deadline(
    path: &Path,
    inspect_nested_mounts: bool,
    deadline: Option<std::time::Instant>,
) -> Result<(), LocalFilesystemError> {
    if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return Err(LocalFilesystemError::Inspection {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "filesystem locality certification exceeded its operation deadline",
            ),
        });
    }
    require_local_filesystem_impl(path, inspect_nested_mounts)
}

fn classify_linux_filesystem_type(filesystem: &str) -> LinuxFilesystemLocality {
    if matches!(
        filesystem,
        "9p" | "afs"
            | "ceph"
            | "ceph-fuse"
            | "cifs"
            | "coda"
            | "davfs"
            | "davfs2"
            | "gfs"
            | "gfs2"
            | "glusterfs"
            | "lustre"
            | "nfs"
            | "nfs4"
            | "ocfs2"
            | "orangefs"
            | "pvfs2"
            | "smbfs"
            | "smb3"
            | "sshfs"
            | "virtiofs"
    ) || filesystem == "fuse"
        || filesystem == "fuseblk"
        || filesystem.starts_with("fuse.")
    {
        return LinuxFilesystemLocality::NonLocal;
    }

    if matches!(
        filesystem,
        "aufs"
            | "bcachefs"
            | "btrfs"
            | "devtmpfs"
            | "ecryptfs"
            | "erofs"
            | "exfat"
            | "ext2"
            | "ext3"
            | "ext4"
            | "f2fs"
            | "iso9660"
            | "jfs"
            | "nilfs2"
            | "ntfs"
            | "ntfs3"
            | "overlay"
            | "ramfs"
            | "reiserfs"
            | "rootfs"
            | "squashfs"
            | "tmpfs"
            | "ubifs"
            | "udf"
            | "vfat"
            | "xfs"
            | "zfs"
    ) {
        LinuxFilesystemLocality::Local
    } else {
        LinuxFilesystemLocality::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn require_local_tree_accepts_a_temporary_directory() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");

        let result = require_local_tree(directory.path());

        assert!(
            result.is_ok(),
            "temporary directory was rejected: {result:?}"
        );
    }

    #[test]
    fn existing_path_gates_reject_an_empty_path_on_every_platform() {
        for result in [
            require_local_tree(Path::new("")),
            require_local_containing_path(Path::new("")),
        ] {
            assert!(matches!(
                result,
                Err(LocalFilesystemError::Inspection { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound
            ));
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn recursive_unix_tree_gate_rejects_descendant_links() {
        use std::os::unix::fs::symlink;

        let tree = tempfile::tempdir().expect("local tree");
        let outside = tempfile::tempdir().expect("outside tree");
        std::fs::write(outside.path().join("remote.go"), "package remote\n")
            .expect("outside source");
        symlink(
            outside.path().join("remote.go"),
            tree.path().join("linked.go"),
        )
        .expect("descendant link");

        let error = require_local_tree(tree.path())
            .expect_err("recursive certification must not follow descendant links");

        assert!(error.to_string().contains("symbolic links"));
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[test]
    fn creation_gate_certifies_parent_without_creating_candidate() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let candidate = directory.path().join("missing/child/cache");

        require_local_filesystem_for_creation(&candidate)
            .expect("temporary directory filesystem should be local");

        assert!(!candidate.exists());
        assert!(!directory.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn containing_gate_certifies_a_symlink_alias_by_its_canonical_target() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let target = directory.path().join("target");
        let alias = directory.path().join("alias");
        std::fs::create_dir(&target).expect("create local target");
        std::os::unix::fs::symlink(&target, &alias).expect("create local alias");

        require_local_containing_path(&alias)
            .expect("a symlink alias to a local target should be certified");
    }

    #[test]
    fn linux_classification_rejects_network_cluster_and_fuse_filesystems() {
        for filesystem in [
            "9p",
            "ceph",
            "cifs",
            "fuse",
            "fuse.sshfs",
            "fuseblk",
            "gfs2",
            "glusterfs",
            "nfs",
            "nfs4",
            "ocfs2",
            "smbfs",
            "smb3",
            "virtiofs",
        ] {
            assert_eq!(
                classify_linux_filesystem_type(filesystem),
                LinuxFilesystemLocality::NonLocal,
                "filesystem type {filesystem} should be rejected"
            );
        }
    }

    #[test]
    fn linux_classification_accepts_common_local_and_ci_filesystems() {
        for filesystem in ["btrfs", "ext4", "overlay", "tmpfs", "xfs"] {
            assert_eq!(
                classify_linux_filesystem_type(filesystem),
                LinuxFilesystemLocality::Local,
                "filesystem type {filesystem} should be accepted"
            );
        }
    }

    #[test]
    fn linux_classification_does_not_assume_an_unknown_filesystem_is_local() {
        assert_eq!(
            classify_linux_filesystem_type("futurefs"),
            LinuxFilesystemLocality::Unknown
        );
    }

    #[cfg(unix)]
    #[test]
    fn linux_mount_selection_uses_the_deepest_containing_mount() {
        let mountinfo = b"20 1 8:1 / / rw - ext4 /dev/root rw\n\
                          21 20 0:42 / /workspace rw - nfs4 server:/workspace rw\n";

        let filesystem = filesystem_type_for_path(mountinfo, Path::new("/workspace/cache"))
            .expect("mount table should parse");

        assert_eq!(filesystem, "nfs4");
    }

    #[cfg(unix)]
    #[test]
    fn linux_mount_selection_rejects_any_nonlocal_peer_in_a_stacked_mount() {
        for mountinfo in [
            b"20 1 8:1 / / rw - ext4 /dev/root rw\n\
              21 20 8:2 / /workspace rw - xfs /dev/data rw\n\
              22 21 0:42 / /workspace rw - nfs4 server:/workspace rw\n"
                .as_slice(),
            b"20 1 8:1 / / rw - ext4 /dev/root rw\n\
              22 21 0:42 / /workspace rw - nfs4 server:/workspace rw\n\
              21 20 8:2 / /workspace rw - xfs /dev/data rw\n"
                .as_slice(),
        ] {
            let mounts = linux_mount_entries(mountinfo).expect("parse stacked mounts");
            let error = certify_linux_containing_mount(&mounts, Path::new("/workspace/cache"))
                .expect_err("every same-point mount must be local");
            assert!(error.to_string().contains("nfs4"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn linux_mount_selection_decodes_mountinfo_path_escapes() {
        let mountinfo = b"20 1 8:1 / / rw - ext4 /dev/root rw\n\
                          21 20 8:2 / /work\\040space rw - xfs /dev/data rw\n";

        let filesystem = filesystem_type_for_path(mountinfo, Path::new("/work space/cache"))
            .expect("mount table should parse");

        assert_eq!(filesystem, "xfs");
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_rejects_a_non_local_destination_before_metadata() {
        let metadata_touched = std::cell::Cell::new(false);

        let error = resolve_unix_path_with(
            Path::new("/remote/cache"),
            UnixPathResolutionMode::Existing,
            None,
            |candidate| {
                if candidate.starts_with("/remote") {
                    Err(LocalFilesystemError::NonLocal {
                        path: candidate.to_path_buf(),
                        reason: "synthetic remote mount".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
            |_, _| Ok(true),
            |_| {
                metadata_touched.set(true);
                Ok(UnixPathKind::Other)
            },
            |_| unreachable!("no symbolic link should be read"),
        )
        .expect_err("the synthetic non-local mount must fail closed");

        assert!(matches!(error, LocalFilesystemError::NonLocal { .. }));
        assert!(!metadata_touched.get());
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_reclassifies_a_symlink_target_before_touching_it() {
        let inspected = std::cell::RefCell::new(Vec::<PathBuf>::new());

        let error = resolve_unix_path_with(
            Path::new("/local/alias"),
            UnixPathResolutionMode::Existing,
            None,
            |candidate| {
                if candidate.starts_with("/remote") {
                    Err(LocalFilesystemError::NonLocal {
                        path: candidate.to_path_buf(),
                        reason: "synthetic remote mount".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
            |_, _| Ok(true),
            |candidate| {
                inspected.borrow_mut().push(candidate.to_path_buf());
                if candidate == Path::new("/local") {
                    Ok(UnixPathKind::Directory)
                } else if candidate == Path::new("/local/alias") {
                    Ok(UnixPathKind::Symlink)
                } else {
                    Ok(UnixPathKind::Other)
                }
            },
            |candidate| {
                assert_eq!(candidate, Path::new("/local/alias"));
                Ok(PathBuf::from("/remote/cache"))
            },
        )
        .expect_err("the redirected non-local mount must fail closed");

        assert!(matches!(error, LocalFilesystemError::NonLocal { .. }));
        assert!(
            inspected
                .borrow()
                .iter()
                .all(|candidate| !candidate.starts_with("/remote"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_rechecks_its_deadline_before_reading_a_symlink() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(10);
        let read_link_touched = std::cell::Cell::new(false);

        let error = resolve_unix_path_with(
            Path::new("/local/alias"),
            UnixPathResolutionMode::Existing,
            Some(deadline),
            |_| Ok(()),
            |_, _| Ok(true),
            |candidate| {
                if candidate == Path::new("/local") {
                    return Ok(UnixPathKind::Directory);
                }
                while std::time::Instant::now() < deadline {
                    std::thread::yield_now();
                }
                Ok(UnixPathKind::Symlink)
            },
            |_| {
                read_link_touched.set(true);
                Ok(PathBuf::from("/remote"))
            },
        )
        .expect_err("an expired deadline must stop before the symlink read");

        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::TimedOut
        ));
        assert!(!read_link_touched.get());
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_applies_parent_components_after_symlink_expansion() {
        let inspected = std::cell::RefCell::new(Vec::<PathBuf>::new());

        let error = resolve_unix_path_with(
            Path::new("/local/link/../repo"),
            UnixPathResolutionMode::Existing,
            None,
            |candidate| {
                if candidate.starts_with("/remote") {
                    Err(LocalFilesystemError::NonLocal {
                        path: candidate.to_path_buf(),
                        reason: "synthetic remote mount".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
            |_, _| Ok(true),
            |candidate| {
                inspected.borrow_mut().push(candidate.to_path_buf());
                if candidate == Path::new("/local") {
                    Ok(UnixPathKind::Directory)
                } else if candidate == Path::new("/local/link") {
                    Ok(UnixPathKind::Symlink)
                } else {
                    Ok(UnixPathKind::Other)
                }
            },
            |candidate| {
                assert_eq!(candidate, Path::new("/local/link"));
                Ok(PathBuf::from("/remote/subdir"))
            },
        )
        .expect_err("the parent component must be applied after the redirect");

        assert!(matches!(error, LocalFilesystemError::NonLocal { .. }));
        assert!(
            inspected
                .borrow()
                .iter()
                .all(|candidate| !candidate.starts_with("/remote"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_bounds_symbolic_link_expansion() {
        let error = resolve_unix_path_with(
            Path::new("/loop"),
            UnixPathResolutionMode::Existing,
            None,
            |_| Ok(()),
            |_, _| Ok(true),
            |_| Ok(UnixPathKind::Symlink),
            |_| Ok(PathBuf::from("/loop")),
        )
        .expect_err("a symbolic-link loop must hit the expansion limit");

        assert!(error.to_string().contains("symbolic-link expansions"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_rejects_oversized_input_before_path_callbacks() {
        use std::os::unix::ffi::OsStringExt;

        let mut bytes = vec![b'/'];
        bytes.resize(MAX_UNIX_PATH_BYTES + 1, b'x');
        let path = PathBuf::from(OsString::from_vec(bytes));
        let callback_touched = std::cell::Cell::new(false);

        let error = resolve_unix_path_with(
            &path,
            UnixPathResolutionMode::Existing,
            None,
            |_| {
                callback_touched.set(true);
                Ok(())
            },
            |_, _| unreachable!("an oversized path must fail before reconciliation"),
            |_| unreachable!("an oversized path must fail before metadata"),
            |_| unreachable!("an oversized path must fail before symlink inspection"),
        )
        .expect_err("an oversized Unix path must fail before it is cloned into components");

        assert!(error.to_string().contains("byte limit"));
        assert!(!callback_touched.get());
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_bounds_cumulative_symlink_target_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let mut bytes = vec![b'/'];
        bytes.resize(MAX_UNIX_PATH_BYTES / 2 + 1, b'x');
        let target = PathBuf::from(OsString::from_vec(bytes));
        let read_count = std::cell::Cell::new(0_usize);

        let error = resolve_unix_path_with(
            Path::new("/loop"),
            UnixPathResolutionMode::Existing,
            None,
            |_| Ok(()),
            |_, _| Ok(true),
            |_| Ok(UnixPathKind::Symlink),
            |_| {
                read_count.set(read_count.get() + 1);
                Ok(target.clone())
            },
        )
        .expect_err("repeated large symlink targets must hit the cumulative byte limit");

        assert!(error.to_string().contains("symlink-target byte limit"));
        assert_eq!(read_count.get(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_preserves_non_unicode_components() {
        use std::os::unix::ffi::OsStringExt;

        let component = OsString::from_vec(vec![b'n', b'o', b'n', 0xFF]);
        let path = Path::new("/local").join(component);

        let resolved = resolve_unix_path_with(
            &path,
            UnixPathResolutionMode::Existing,
            None,
            |_| Ok(()),
            |_, _| Ok(true),
            |candidate| {
                if candidate == Path::new("/local") {
                    Ok(UnixPathKind::Directory)
                } else {
                    Ok(UnixPathKind::Other)
                }
            },
            |_| unreachable!("no symbolic link should be read"),
        )
        .expect("non-Unicode Unix components must remain byte-preserving");

        assert_eq!(resolved, path);
    }

    #[cfg(unix)]
    #[test]
    fn unix_resolution_rejects_a_case_alias_before_inspecting_a_mount_boundary() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let parent = std::fs::canonicalize(directory.path())
            .expect("temporary directory should have a canonical path");
        let stored_mount = parent.join("RemoteBoundary");
        std::fs::create_dir(&stored_mount).expect("synthetic mount directory should be created");
        let requested_mount = parent.join("remoteboundary");
        let requested_path = requested_mount.join("child");
        let inspected = std::cell::RefCell::new(Vec::<PathBuf>::new());
        let mut reconciliation_budget = UnixReconciliationBudget::new(None);

        let error = resolve_unix_path_with(
            &requested_path,
            UnixPathResolutionMode::Existing,
            None,
            |_| Ok(()),
            |lookup_parent, component| {
                exact_unix_component_for_mounts(
                    lookup_parent,
                    component,
                    std::iter::once(stored_mount.as_path()),
                    &mut reconciliation_budget,
                )
            },
            |candidate| {
                inspected.borrow_mut().push(candidate.to_path_buf());
                inspect_unix_path_kind(candidate)
            },
            |candidate| std::fs::read_link(candidate),
        )
        .expect_err("a non-exact mount-boundary spelling must fail closed");

        assert!(error.to_string().contains("mount boundary"));
        assert!(
            inspected
                .borrow()
                .iter()
                .all(|candidate| candidate != &requested_mount),
            "the aliased mount boundary was inspected before rejection"
        );
    }

    #[cfg(unix)]
    #[test]
    fn creation_gate_rejects_parent_traversal_after_a_missing_component() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let remote = directory.path().join("remote");
        std::fs::create_dir(&remote).expect("create escape destination");
        let candidate = directory.path().join("missing/../remote/new");

        let result = require_local_filesystem_for_creation(&candidate);
        if result.is_ok() {
            std::fs::create_dir_all(&candidate)
                .expect("an incorrectly accepted creation path would be followed");
        }

        let error = result.expect_err("creation must fail before following the parent component");
        assert!(error.to_string().contains("parent"));
        assert!(!directory.path().join("missing").exists());
        assert!(!remote.join("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn unix_component_reconciliation_preserves_distinct_exact_siblings() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let requested = directory.path().join("LocalSibling");
        let mounted = directory.path().join("RemoteSibling");
        std::fs::create_dir(&requested).expect("local sibling should be created");
        std::fs::create_dir(&mounted).expect("synthetic mount sibling should be created");
        let mut reconciliation_budget = UnixReconciliationBudget::new(None);

        let exact = exact_unix_component_for_mounts(
            directory.path(),
            std::ffi::OsStr::new("LocalSibling"),
            std::iter::once(mounted.as_path()),
            &mut reconciliation_budget,
        )
        .expect("distinct exact siblings should not be treated as aliases");

        assert!(exact);
    }

    #[cfg(unix)]
    #[test]
    fn unix_component_reconciliation_rejects_unicode_normalization_ambiguity() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let decomposed = "Cafe\u{301}";
        let composed = "Caf\u{e9}";
        let mounted = directory.path().join(decomposed);
        std::fs::create_dir(&mounted).expect("synthetic Unicode mount should be created");
        let mut reconciliation_budget = UnixReconciliationBudget::new(None);

        let error = exact_unix_component_for_mounts(
            directory.path(),
            std::ffi::OsStr::new(composed),
            std::iter::once(mounted.as_path()),
            &mut reconciliation_budget,
        )
        .expect_err("normalization-equivalent mount spelling must fail closed");

        assert!(error.to_string().contains("mount"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_reconciliation_budget_is_cumulative_and_deadline_aware() {
        let mut entry_budget = UnixReconciliationBudget::new(None);
        entry_budget.directory_entries = MAX_UNIX_RECONCILIATION_ENTRIES_TOTAL;
        let entry_error = entry_budget
            .observe_directory_entry(1)
            .expect_err("the cumulative entry limit must fail closed");
        assert_eq!(entry_error.kind(), std::io::ErrorKind::InvalidData);

        let mut mount_budget = UnixReconciliationBudget::new(None);
        mount_budget.mount_records = MAX_UNIX_RECONCILIATION_MOUNT_RECORDS_TOTAL;
        let mount_error = mount_budget
            .observe_mount_record()
            .expect_err("the cumulative mount-record limit must fail closed");
        assert_eq!(mount_error.kind(), std::io::ErrorKind::InvalidData);

        let mut expired = UnixReconciliationBudget::new(Some(std::time::Instant::now()));
        let timeout = expired
            .observe_directory_entry(1)
            .expect_err("an expired reconciliation deadline must stop enumeration");
        assert_eq!(timeout.kind(), std::io::ErrorKind::TimedOut);
    }

    #[cfg(unix)]
    #[test]
    fn unix_root_certification_honors_an_already_expired_deadline() {
        let error = require_local_tree_until(Path::new("/"), std::time::Instant::now())
            .expect_err("an expired deadline must stop before mount-table work");
        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::TimedOut
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unix_creation_certification_honors_an_already_expired_deadline() {
        let error = require_local_filesystem_for_creation_until(
            Path::new("/polint-deadline-probe"),
            std::time::Instant::now(),
        )
        .expect_err("an expired deadline must stop before mount-table work");
        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::TimedOut
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unix_locality_gates_reject_an_empty_path() {
        assert!(matches!(
            require_local_containing_path(Path::new("")),
            Err(LocalFilesystemError::Inspection { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[test]
    fn creation_gate_rejects_an_empty_path_on_every_platform() {
        assert!(matches!(
            require_local_filesystem_for_creation(Path::new("")),
            Err(LocalFilesystemError::Inspection { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(unix)]
    #[test]
    fn containing_gate_rejects_parent_after_a_regular_file() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let file = directory.path().join("regular");
        std::fs::write(&file, b"content").expect("regular file should be created");

        let error = require_local_containing_path(&file.join(".."))
            .expect_err("a parent step through a regular file must fail");

        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::NotADirectory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn containing_gate_rejects_current_directory_after_a_regular_file() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let file = directory.path().join("regular");
        std::fs::write(&file, b"content").expect("regular file should be created");

        let error = require_local_containing_path(&file.join("."))
            .expect_err("a current-directory step through a regular file must fail");

        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::NotADirectory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn containing_gate_rejects_a_trailing_separator_after_a_regular_file() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let file = directory.path().join("regular");
        std::fs::write(&file, b"content").expect("regular file should be created");
        let mut path_bytes = file.as_os_str().as_bytes().to_vec();
        path_bytes.push(b'/');
        let path = PathBuf::from(OsString::from_vec(path_bytes));

        let error = require_local_containing_path(&path)
            .expect_err("a trailing separator after a regular file must fail");

        assert!(matches!(
            error,
            LocalFilesystemError::Inspection { source, .. }
                if source.kind() == std::io::ErrorKind::NotADirectory
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_snapshot_bounds_the_initial_count_before_allocating() {
        let read_was_called = std::cell::Cell::new(false);

        let error = caller_owned_bsd_mount_snapshot::<u8>(
            || Ok(MAX_BSD_MOUNT_ENTRIES + 1),
            |_| {
                read_was_called.set(true);
                Ok(Vec::new())
            },
            None,
        )
        .expect_err("an oversized mount count must fail before allocation");

        assert!(error.to_string().contains("entry limit"));
        assert!(!read_was_called.get());
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_fields_reject_the_truncation_boundary() {
        let boundary_terminated = [b'x' as libc::c_char, 0];
        let error = bounded_bsd_c_string(&boundary_terminated, "mount point")
            .expect_err("a final-slot terminator is indistinguishable from truncation");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);

        let complete = [b'x' as libc::c_char, 0, 0];
        assert_eq!(
            bounded_bsd_c_string(&complete, "mount point").expect("complete mount field"),
            b"x"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_snapshot_grows_when_the_mount_table_fills_its_buffer() {
        let capacities = std::cell::RefCell::new(Vec::new());
        let read_count = std::cell::Cell::new(0_usize);

        let mounts = caller_owned_bsd_mount_snapshot(
            || Ok(1),
            |capacity| {
                capacities.borrow_mut().push(capacity);
                let current = read_count.get();
                read_count.set(current + 1);
                if current == 0 {
                    Ok(vec![7_u8; capacity])
                } else {
                    Ok(vec![7_u8, 8_u8])
                }
            },
            None,
        )
        .expect("a mount table that grows within the bound should be retried");

        assert_eq!(mounts, [7, 8]);
        assert_eq!(*capacities.borrow(), [16, 32]);
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_snapshot_rechecks_its_deadline_between_queries() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(10);
        let read_was_called = std::cell::Cell::new(false);

        let error = caller_owned_bsd_mount_snapshot::<u8>(
            || {
                while std::time::Instant::now() < deadline {
                    std::thread::yield_now();
                }
                Ok(1)
            },
            |_| {
                read_was_called.set(true);
                Ok(vec![1])
            },
            Some(deadline),
        )
        .expect_err("an expired deadline must stop before the mount-table read");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(!read_was_called.get());
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_snapshot_propagates_count_and_read_errors() {
        let count_error = caller_owned_bsd_mount_snapshot::<u8>(
            || Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            |_| unreachable!("a failed count must not read the mount table"),
            None,
        )
        .expect_err("a mount count error must be preserved");
        assert_eq!(count_error.kind(), std::io::ErrorKind::PermissionDenied);

        let read_error = caller_owned_bsd_mount_snapshot::<u8>(
            || Ok(1),
            |_| Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
            None,
        )
        .expect_err("a mount snapshot read error must be preserved");
        assert_eq!(read_error.kind(), std::io::ErrorKind::Interrupted);
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    #[test]
    fn bsd_mount_snapshot_reads_the_root_into_caller_owned_storage() {
        let mounts = bsd_mount_entries(Path::new("/"), None)
            .expect("the platform mount table should produce an owned snapshot");

        assert!(
            mounts
                .iter()
                .any(|mount| mount.mount_point == Path::new("/")),
            "the owned mount snapshot did not contain the root mount"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_rejects_a_non_local_nested_mount() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/workspace", "zfs", true),
            bsd_mount("/workspace/vendor", "nfs", false),
        ];

        let error = certify_bsd_mounts(Path::new("/workspace"), &mounts, true)
            .expect_err("a non-local nested mount should be rejected");

        assert!(matches!(
            error,
            LocalFilesystemError::NonLocal { path, reason }
                if path == Path::new("/workspace/vendor") && reason.contains("`nfs`")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_ignores_a_non_local_sibling_mount() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/Users/example", "apfs", true),
            bsd_mount("/Users/example/OrbStack", "nfs", false),
        ];

        let result = certify_bsd_mounts(Path::new("/Users/example/.cache/polint"), &mounts, true);

        assert!(
            result.is_ok(),
            "a sibling mount outside the traversed cache tree was rejected: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_uses_path_component_boundaries() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/workspace/repo2", "nfs", false),
        ];

        let result = certify_bsd_mounts(Path::new("/workspace/repo"), &mounts, true);

        assert!(
            result.is_ok(),
            "a lexical prefix outside the traversed path was treated as a child mount: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_rejects_a_non_local_containing_mount() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/network", "nfs", false),
        ];

        let error = certify_bsd_mounts(Path::new("/network/project"), &mounts, false)
            .expect_err("a non-local containing mount should be rejected");

        assert!(matches!(
            error,
            LocalFilesystemError::NonLocal { path, reason }
                if path == Path::new("/network/project") && reason.contains("`nfs`")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_rejects_any_nonlocal_peer_in_a_stacked_mount() {
        for mounts in [
            [
                bsd_mount("/", "ffs", true),
                bsd_mount("/workspace", "zfs", true),
                bsd_mount("/workspace", "nfs", false),
            ],
            [
                bsd_mount("/", "ffs", true),
                bsd_mount("/workspace", "nfs", false),
                bsd_mount("/workspace", "zfs", true),
            ],
        ] {
            let error = certify_bsd_mounts(Path::new("/workspace/project"), &mounts, false)
                .expect_err("every same-point BSD mount must be local");
            assert!(error.to_string().contains("nfs"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn bsd_mount_certification_allows_local_nested_mounts() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/workspace", "zfs", true),
            bsd_mount("/workspace/vendor", "tmpfs", true),
        ];

        let result = certify_bsd_mounts(Path::new("/workspace"), &mounts, true);

        assert!(result.is_ok(), "local mount tree was rejected: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn bsd_creation_certification_does_not_inspect_descendant_mounts() {
        let mounts = [
            bsd_mount("/", "ffs", true),
            bsd_mount("/workspace", "zfs", true),
            bsd_mount("/workspace/vendor", "nfs", false),
        ];

        let result = certify_bsd_mounts(Path::new("/workspace"), &mounts, false);

        assert!(
            result.is_ok(),
            "non-recursive certification rejected a descendant: {result:?}"
        );
    }

    #[cfg(unix)]
    fn bsd_mount(path: &str, filesystem: &str, local: bool) -> BsdMountEntry {
        BsdMountEntry {
            mount_point: PathBuf::from(path),
            filesystem: filesystem.to_owned(),
            local,
        }
    }
}
