#[cfg(unix)]
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use thiserror::Error;

pub(crate) const TOPOLOGY_MANIFEST_MAX_BYTES: u64 = 1_048_576;
pub(crate) const TOPOLOGY_LOCKFILE_MAX_BYTES: u64 = 16 * 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoFileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    file_attributes: u32,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    file_size: u64,
    #[cfg(not(any(unix, windows)))]
    length: u64,
    #[cfg(not(any(unix, windows)))]
    modified: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoFileSnapshot {
    pub(crate) identity: RepoFileIdentity,
    pub(crate) contents: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RepoCreatedDirectory {
    pub(crate) relative_path: PathBuf,
    identity: RepoFileIdentity,
}

#[derive(Debug, Error)]
pub(crate) enum RepoFileReadError {
    #[error("absolute path")]
    AbsolutePath,
    #[error("path escapes repository root")]
    EscapesRepo,
    #[error("repository root unavailable")]
    RootUnavailable,
    #[error("not found")]
    NotFound,
    #[error("not a file")]
    NotFile,
    #[error("not a directory")]
    NotDirectory,
    #[error("metadata unavailable")]
    Metadata,
    #[error("file exceeds topology input size limit")]
    TooLarge { max_bytes: u64 },
    #[error("invalid utf-8")]
    InvalidUtf8,
    #[error("read failed")]
    Read,
    #[error("create directory failed")]
    CreateDir,
    #[error("write failed")]
    Write,
    #[error("atomic persist failed")]
    Persist,
    #[error("destination changed concurrently")]
    ConcurrentModification,
}

impl RepoFileReadError {
    pub(crate) fn stable_reason(&self) -> &'static str {
        match self {
            Self::AbsolutePath => "absolute path",
            Self::EscapesRepo => "path escapes repository root",
            Self::RootUnavailable => "repository root unavailable",
            Self::NotFound => "not found",
            Self::NotFile => "not a file",
            Self::NotDirectory => "not a directory",
            Self::Metadata => "metadata unavailable",
            Self::TooLarge { max_bytes } => {
                let _ = max_bytes;
                "file exceeds topology input size limit"
            }
            Self::InvalidUtf8 => "invalid utf-8",
            Self::Read => "read failed",
            Self::CreateDir => "directory creation failed",
            Self::Write => "write failed",
            Self::Persist => "atomic persist failed",
            Self::ConcurrentModification => "destination changed concurrently",
        }
    }

    pub(crate) fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }
}

pub(crate) fn normalize_repo_relative(path: impl AsRef<str>) -> Option<String> {
    let path = path.as_ref().replace('\\', "/");
    let mut parts = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            segment => parts.push(segment),
        }
    }

    if parts.is_empty() {
        Some(".".to_string())
    } else {
        Some(parts.join("/"))
    }
}

pub(crate) fn normalize_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    Some(normalized)
}

pub(crate) fn normalize_repo_relative_input(path: impl AsRef<Path>) -> Option<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(normalized)
}

pub(crate) fn repo_file_path(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, RepoFileReadError> {
    let target = canonical_repo_target(root, relative_path)?;
    let metadata = fs::metadata(&target).map_err(|_| RepoFileReadError::Metadata)?;
    if !metadata.is_file() {
        return Err(RepoFileReadError::NotFile);
    }
    Ok(target)
}

pub(crate) fn repo_dir_path(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, RepoFileReadError> {
    let target = canonical_repo_target(root, relative_path)?;
    let metadata = fs::metadata(&target).map_err(|_| RepoFileReadError::Metadata)?;
    if !metadata.is_dir() {
        return Err(RepoFileReadError::NotDirectory);
    }
    Ok(target)
}

pub(crate) fn repo_file_exists(root: &Path, relative_path: impl AsRef<Path>) -> bool {
    repo_file_path(root, relative_path).is_ok()
}

pub(crate) fn read_repo_file(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<Vec<u8>, RepoFileReadError> {
    let target = repo_file_path(root, relative_path)?;
    fs::read(target).map_err(|_| RepoFileReadError::Read)
}

pub(crate) fn read_repo_file_with_limit(
    root: &Path,
    relative_path: impl AsRef<Path>,
    max_bytes: u64,
) -> Result<Vec<u8>, RepoFileReadError> {
    if max_bytes == u64::MAX {
        return read_repo_file(root, relative_path);
    }
    let target = repo_file_path(root, relative_path)?;
    read_file_with_limit(&target, max_bytes)
}

pub(crate) fn read_file_with_limit(
    target: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, RepoFileReadError> {
    let metadata = fs::metadata(target).map_err(|_| RepoFileReadError::Metadata)?;
    if metadata.len() > max_bytes {
        return Err(RepoFileReadError::TooLarge { max_bytes });
    }
    let mut file = fs::File::open(target).map_err(|_| RepoFileReadError::Read)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| RepoFileReadError::Read)?;
    if bytes.len() as u64 > max_bytes {
        return Err(RepoFileReadError::TooLarge { max_bytes });
    }
    Ok(bytes)
}

pub(crate) fn read_file_to_string_with_limit(
    target: &Path,
    max_bytes: u64,
) -> Result<String, RepoFileReadError> {
    let bytes = read_file_with_limit(target, max_bytes)?;
    String::from_utf8(bytes).map_err(|_| RepoFileReadError::InvalidUtf8)
}

pub(crate) fn read_repo_file_to_string_with_limit(
    root: &Path,
    relative_path: impl AsRef<Path>,
    max_bytes: u64,
) -> Result<String, RepoFileReadError> {
    let bytes = read_repo_file_with_limit(root, relative_path, max_bytes)?;
    String::from_utf8(bytes).map_err(|_| RepoFileReadError::InvalidUtf8)
}

pub(crate) fn repo_relative_existing_path(root: &Path, path: &Path) -> Option<String> {
    let root = root.canonicalize().ok()?;
    let path = path.canonicalize().ok()?;
    let relative = path.strip_prefix(root).ok()?;
    normalize_repo_relative(relative.to_string_lossy())
}

pub(crate) fn ensure_repo_dir(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, RepoFileReadError> {
    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    let root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let mut current = root.clone();
    for component in relative_path.components() {
        let Component::Normal(segment) = component else {
            return Err(RepoFileReadError::AbsolutePath);
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoFileReadError::EscapesRepo);
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(RepoFileReadError::NotDirectory),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // The probe above and this create are not atomic, so a
                // concurrent writer building the same managed directory wins
                // the race and this call sees `AlreadyExists`. That is the
                // other writer succeeding, not a failure: treating it as one
                // fails the whole write and emits a spurious `internal/cache`
                // warning on cold parallel runs. Re-stat the entry the winner
                // left so the symlink-escape and is-a-directory guarantees
                // still hold for what we are about to write into.
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let metadata = fs::symlink_metadata(&current)
                            .map_err(|_| RepoFileReadError::Metadata)?;
                        if metadata.file_type().is_symlink() {
                            return Err(RepoFileReadError::EscapesRepo);
                        }
                        if !metadata.is_dir() {
                            return Err(RepoFileReadError::NotDirectory);
                        }
                    }
                    Err(_) => return Err(RepoFileReadError::CreateDir),
                }
            }
            Err(_) => return Err(RepoFileReadError::Metadata),
        }
        let canonical_current = current
            .canonicalize()
            .map_err(|_| RepoFileReadError::Metadata)?;
        if !canonical_current.starts_with(&root) {
            return Err(RepoFileReadError::EscapesRepo);
        }
    }
    Ok(current)
}

pub(crate) fn repo_write_target(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, RepoFileReadError> {
    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    if relative_path.as_os_str().is_empty() {
        return Err(RepoFileReadError::NotFile);
    }
    let root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let mut current = root.clone();
    let component_count = relative_path.components().count();

    for (index, component) in relative_path.components().enumerate() {
        let Component::Normal(segment) = component else {
            return Err(RepoFileReadError::AbsolutePath);
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoFileReadError::EscapesRepo);
            }
            Ok(metadata) if index + 1 < component_count && !metadata.is_dir() => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Ok(_) => {
                let canonical_current = current
                    .canonicalize()
                    .map_err(|_| RepoFileReadError::Metadata)?;
                if !canonical_current.starts_with(&root) {
                    return Err(RepoFileReadError::EscapesRepo);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Err(_) => return Err(RepoFileReadError::Metadata),
        }
    }

    Ok(root.join(relative_path))
}

fn metadata_identity(metadata: &fs::Metadata) -> RepoFileIdentity {
    #[cfg(unix)]
    {
        RepoFileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(windows)]
    {
        RepoFileIdentity {
            file_attributes: metadata.file_attributes(),
            creation_time: metadata.creation_time(),
            last_write_time: metadata.last_write_time(),
            file_size: metadata.file_size(),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        RepoFileIdentity {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

#[cfg(unix)]
struct RepoParentDir {
    root: fs::File,
    parent: fs::File,
    components: Vec<OsString>,
    identities: Vec<RepoFileIdentity>,
    target_name: OsString,
}

#[cfg(unix)]
fn open_repo_parent(
    root: &Path,
    relative_path: impl AsRef<Path>,
    create: bool,
) -> Result<RepoParentDir, RepoFileReadError> {
    open_repo_parent_tracked(root, relative_path, create, None)
}

#[cfg(unix)]
fn open_repo_parent_tracked(
    root: &Path,
    relative_path: impl AsRef<Path>,
    create: bool,
    mut created_directories: Option<&mut Vec<RepoCreatedDirectory>>,
) -> Result<RepoParentDir, RepoFileReadError> {
    use rustix::fs::{Mode, OFlags};

    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    let mut components = relative_path
        .components()
        .map(|component| match component {
            Component::Normal(segment) => Ok(segment.to_owned()),
            _ => Err(RepoFileReadError::AbsolutePath),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_name = components.pop().ok_or(RepoFileReadError::NotFile)?;

    let canonical_root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let root_fd = rustix::fs::open(
        &canonical_root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let root = fs::File::from(root_fd);
    let mut current = root.try_clone().map_err(|_| RepoFileReadError::Metadata)?;
    let mut identities = Vec::with_capacity(components.len());
    let mut relative_directory = PathBuf::new();

    for component in &components {
        relative_directory.push(component);
        let mut created_here = false;
        let opened = match rustix::fs::openat(
            &current,
            component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(error) if error == rustix::io::Errno::NOENT && create => {
                match rustix::fs::mkdirat(&current, component, Mode::RWXU | Mode::RWXG | Mode::RWXO)
                {
                    Ok(()) => created_here = true,
                    Err(error) if error == rustix::io::Errno::EXIST => {}
                    Err(_) => return Err(RepoFileReadError::CreateDir),
                }
                rustix::fs::openat(
                    &current,
                    component,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|error| map_repo_directory_open_error(&current, component, error))?
            }
            Err(error) => {
                return Err(map_repo_directory_open_error(&current, component, error));
            }
        };
        let opened = fs::File::from(opened);
        let identity =
            metadata_identity(&opened.metadata().map_err(|_| RepoFileReadError::Metadata)?);
        if created_here && let Some(created_directories) = created_directories.as_deref_mut() {
            created_directories.push(RepoCreatedDirectory {
                relative_path: relative_directory.clone(),
                identity: identity.clone(),
            });
        }
        identities.push(identity);
        current = opened;
    }

    Ok(RepoParentDir {
        root,
        parent: current,
        components,
        identities,
        target_name,
    })
}
#[cfg(unix)]
fn map_repo_directory_open_error(
    parent: &fs::File,
    component: &OsStr,
    error: rustix::io::Errno,
) -> RepoFileReadError {
    use rustix::fs::{AtFlags, FileType, statat};

    if error == rustix::io::Errno::NOENT {
        RepoFileReadError::NotFound
    } else if error == rustix::io::Errno::LOOP {
        RepoFileReadError::EscapesRepo
    } else if error == rustix::io::Errno::NOTDIR {
        match statat(parent, component, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink => {
                RepoFileReadError::EscapesRepo
            }
            _ => RepoFileReadError::NotDirectory,
        }
    } else {
        RepoFileReadError::Metadata
    }
}

#[cfg(unix)]
fn rooted_repo_parent(parent: &RepoParentDir) -> Result<fs::File, RepoFileReadError> {
    use rustix::fs::{Mode, OFlags};

    let mut current = parent
        .root
        .try_clone()
        .map_err(|_| RepoFileReadError::Metadata)?;
    for (component, expected_identity) in parent.components.iter().zip(&parent.identities) {
        let opened = rustix::fs::openat(
            &current,
            component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| map_repo_directory_open_error(&current, component, error))?;
        let opened = fs::File::from(opened);
        let identity =
            metadata_identity(&opened.metadata().map_err(|_| RepoFileReadError::Metadata)?);
        if identity != *expected_identity {
            return Err(RepoFileReadError::EscapesRepo);
        }
        current = opened;
    }
    let current_identity = metadata_identity(
        &current
            .metadata()
            .map_err(|_| RepoFileReadError::Metadata)?,
    );
    let parent_identity = metadata_identity(
        &parent
            .parent
            .metadata()
            .map_err(|_| RepoFileReadError::Metadata)?,
    );
    if current_identity != parent_identity {
        return Err(RepoFileReadError::EscapesRepo);
    }

    // This freshly walked descriptor is the mutation capability. Every component
    // was selected below the held repository-root descriptor without following
    // symlinks. If an ancestor is renamed after this walk, fd-relative syscalls
    // still address the selected directory inode; the rename cannot redirect the
    // operation through a replacement path or symlink.
    Ok(current)
}

#[cfg(unix)]
fn ensure_repo_parent_rooted(parent: &RepoParentDir) -> Result<(), RepoFileReadError> {
    rooted_repo_parent(parent).map(drop)
}
#[cfg(not(unix))]
fn file_snapshot(target: &Path) -> Result<RepoFileSnapshot, RepoFileReadError> {
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RepoFileReadError::NotFound
        } else {
            RepoFileReadError::Metadata
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RepoFileReadError::EscapesRepo);
    }
    if !metadata.is_file() {
        return Err(RepoFileReadError::NotFile);
    }
    let identity = metadata_identity(&metadata);
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options.open(target).map_err(|_| RepoFileReadError::Read)?;
    let opened_identity =
        metadata_identity(&file.metadata().map_err(|_| RepoFileReadError::Metadata)?);
    if opened_identity != identity {
        return Err(RepoFileReadError::Metadata);
    }
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|_| RepoFileReadError::Read)?;
    Ok(RepoFileSnapshot { identity, contents })
}

#[cfg(unix)]
pub(crate) fn read_optional_repo_file_snapshot(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<Option<RepoFileSnapshot>, RepoFileReadError> {
    let parent = match open_repo_parent(root, relative_path, false) {
        Ok(parent) => parent,
        Err(RepoFileReadError::NotFound) => return Ok(None),
        Err(error) => return Err(error),
    };
    let snapshot = match file_snapshot_at(&parent.parent, &parent.target_name) {
        Ok(snapshot) => Some(snapshot),
        Err(RepoFileReadError::NotFound) => None,
        Err(error) => return Err(error),
    };
    ensure_repo_parent_rooted(&parent)?;
    Ok(snapshot)
}

#[cfg(not(unix))]
pub(crate) fn read_optional_repo_file_snapshot(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<Option<RepoFileSnapshot>, RepoFileReadError> {
    let target = repo_write_target(root, relative_path)?;
    match file_snapshot(&target) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(RepoFileReadError::NotFound) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn write_repo_file_atomic(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), RepoFileReadError> {
    write_repo_file_atomic_impl(root, relative_path, contents, false, None).map(|_| ())
}

#[cfg(test)]
fn write_repo_file_atomic_noclobber(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), RepoFileReadError> {
    write_repo_file_atomic_impl(root, relative_path, contents, true, None).map(|_| ())
}

pub(crate) fn write_repo_file_atomic_tracked(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    expected: &RepoFileSnapshot,
    created_directories: &mut Vec<RepoCreatedDirectory>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    write_repo_file_atomic_impl_tracked(
        root,
        relative_path,
        contents,
        false,
        Some(expected),
        created_directories,
    )
}

pub(crate) fn write_repo_file_atomic_noclobber_tracked(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    created_directories: &mut Vec<RepoCreatedDirectory>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    write_repo_file_atomic_impl_tracked(
        root,
        relative_path,
        contents,
        true,
        None,
        created_directories,
    )
}

#[cfg(unix)]
fn write_repo_file_atomic_impl(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    write_repo_file_atomic_impl_with_hook(root, relative_path, contents, noclobber, expected, || {})
}

#[cfg(unix)]
fn write_repo_file_atomic_impl_tracked(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    created_directories: &mut Vec<RepoCreatedDirectory>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    let parent = open_repo_parent_tracked(root, relative_path, true, Some(created_directories))?;
    ensure_repo_target_is_file_or_missing(&parent)?;
    write_repo_file_atomic_at_with_hook(&parent, contents, noclobber, expected, || {})
}

#[cfg(unix)]
fn write_repo_file_atomic_impl_with_hook<F>(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    before_commit: F,
) -> Result<RepoFileIdentity, RepoFileReadError>
where
    F: FnOnce(),
{
    let parent = open_repo_parent(root, relative_path, true)?;
    ensure_repo_target_is_file_or_missing(&parent)?;
    write_repo_file_atomic_at_with_hook(&parent, contents, noclobber, expected, before_commit)
}

#[cfg(not(unix))]
fn write_repo_file_atomic_impl(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    if relative_path.as_os_str().is_empty() {
        return Err(RepoFileReadError::NotFile);
    }
    let parent_relative = relative_path.parent().unwrap_or_else(|| Path::new("."));
    let parent = ensure_repo_dir(root, parent_relative)?;
    let root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let target = root.join(&relative_path);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(RepoFileReadError::EscapesRepo);
        }
        Ok(metadata) if metadata.is_dir() => return Err(RepoFileReadError::NotFile),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(if error.kind() == std::io::ErrorKind::NotADirectory {
                RepoFileReadError::NotDirectory
            } else {
                RepoFileReadError::Metadata
            });
        }
    }
    write_file_atomic_in_existing_dir(&parent, &target, contents, noclobber, expected)
}

#[cfg(not(unix))]
fn write_repo_file_atomic_impl_tracked(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    created_directories: &mut Vec<RepoCreatedDirectory>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    let _ = (
        root,
        relative_path,
        contents,
        noclobber,
        expected,
        created_directories,
    );
    Err(RepoFileReadError::Persist)
}

#[cfg(unix)]
fn unique_quarantine_name() -> OsString {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_QUARANTINE: AtomicU64 = AtomicU64::new(0);
    let counter = NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!(
        ".polint-quarantine-{}-{nonce:x}-{counter:x}",
        std::process::id()
    )
    .into()
}

#[cfg(all(
    unix,
    any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )
))]
fn renameat_noreplace(
    parent: &fs::File,
    old_name: &OsStr,
    new_name: &OsStr,
) -> rustix::io::Result<()> {
    rustix::fs::renameat_with(
        parent,
        old_name,
        parent,
        new_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
}

#[cfg(all(
    unix,
    not(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))
))]
fn renameat_noreplace(
    _parent: &fs::File,
    _old_name: &OsStr,
    _new_name: &OsStr,
) -> rustix::io::Result<()> {
    // Safe compare-and-mutate requires an atomic no-replace rename. Platforms
    // without that capability must refuse rollback rather than emulate it with
    // a check followed by a clobbering rename.
    Err(rustix::io::Errno::NOTSUP)
}

#[cfg(unix)]
fn quarantine_repo_target<F>(
    parent: &RepoParentDir,
    before_quarantine: F,
) -> Result<(fs::File, OsString), RepoFileReadError>
where
    F: FnOnce(),
{
    before_quarantine();

    // Rewalk after the last caller-controlled hook and use the resulting
    // capability for the complete quarantine transaction. There is no
    // check-to-path-mutation gap: the rename is fd-relative to the directory
    // inode selected from the repository root.
    let transaction_parent = rooted_repo_parent(parent)?;
    for _ in 0..128 {
        let quarantine_name = unique_quarantine_name();
        match renameat_noreplace(&transaction_parent, &parent.target_name, &quarantine_name) {
            Ok(()) => return Ok((transaction_parent, quarantine_name)),
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) if error == rustix::io::Errno::NOENT => {
                return Err(RepoFileReadError::NotFound);
            }
            Err(_) => return Err(RepoFileReadError::Write),
        }
    }
    Err(RepoFileReadError::Write)
}

#[cfg(unix)]
fn restore_quarantine_noclobber(
    parent: &fs::File,
    quarantine_name: &OsStr,
    target_name: &OsStr,
) -> Result<bool, RepoFileReadError> {
    use rustix::fs::AtFlags;

    match rustix::fs::linkat(
        parent,
        quarantine_name,
        parent,
        target_name,
        AtFlags::empty(),
    ) {
        Ok(()) => {
            unlink_quarantine(parent, quarantine_name)?;
            Ok(true)
        }
        Err(error) if error == rustix::io::Errno::EXIST => Ok(false),
        Err(_) => Err(RepoFileReadError::Write),
    }
}

#[cfg(unix)]
fn restore_directory_quarantine_noclobber(
    parent: &fs::File,
    quarantine_name: &OsStr,
    target_name: &OsStr,
) -> Result<bool, RepoFileReadError> {
    match renameat_noreplace(parent, quarantine_name, target_name) {
        Ok(()) => Ok(true),
        Err(error) if error == rustix::io::Errno::EXIST => Ok(false),
        Err(_) => Err(RepoFileReadError::Write),
    }
}

#[cfg(unix)]
fn unlink_quarantine(parent: &fs::File, quarantine_name: &OsStr) -> Result<(), RepoFileReadError> {
    rustix::fs::unlinkat(parent, quarantine_name, rustix::fs::AtFlags::empty())
        .map_err(|_| RepoFileReadError::Write)
}

#[cfg(unix)]
pub(crate) fn remove_repo_file_if_matches(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
) -> Result<bool, RepoFileReadError> {
    remove_repo_file_if_matches_with_hook(root, relative_path, expected, || {})
}

#[cfg(unix)]
fn remove_repo_file_if_matches_with_hook<F>(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
    before_quarantine: F,
) -> Result<bool, RepoFileReadError>
where
    F: FnOnce(),
{
    let parent = open_repo_parent(root, relative_path, false)?;
    let (transaction_parent, quarantine_name) =
        match quarantine_repo_target(&parent, before_quarantine) {
            Ok(quarantine) => quarantine,
            Err(RepoFileReadError::NotFound) => return Ok(false),
            Err(error) => return Err(error),
        };

    match file_snapshot_at(&transaction_parent, &quarantine_name) {
        Ok(quarantined) if quarantined == *expected => {
            unlink_quarantine(&transaction_parent, &quarantine_name)?;
            Ok(true)
        }
        Ok(_) | Err(RepoFileReadError::NotFile | RepoFileReadError::EscapesRepo) => {
            if restore_quarantine_noclobber(
                &transaction_parent,
                &quarantine_name,
                &parent.target_name,
            )? {
                Ok(false)
            } else {
                Err(RepoFileReadError::ConcurrentModification)
            }
        }
        Err(error) => {
            if restore_quarantine_noclobber(
                &transaction_parent,
                &quarantine_name,
                &parent.target_name,
            )? {
                Err(error)
            } else {
                Err(RepoFileReadError::ConcurrentModification)
            }
        }
    }
}

#[cfg(not(unix))]
pub(crate) fn remove_repo_file_if_matches(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
) -> Result<bool, RepoFileReadError> {
    let target = repo_write_target(root, relative_path)?;
    for _ in 0..2 {
        match file_snapshot(&target) {
            Ok(current) if current == *expected => {}
            Ok(_) | Err(RepoFileReadError::NotFound) => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    fs::remove_file(target)
        .map(|()| true)
        .map_err(|_| RepoFileReadError::Write)
}

#[cfg(unix)]
pub(crate) fn restore_repo_file_if_matches(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
    replacement: &[u8],
) -> Result<bool, RepoFileReadError> {
    match write_repo_file_atomic_impl(root, relative_path, replacement, false, Some(expected)) {
        Ok(_) => Ok(true),
        Err(RepoFileReadError::ConcurrentModification) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(all(unix, test))]
fn restore_repo_file_if_matches_with_hook<F>(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
    replacement: &[u8],
    before_quarantine: F,
) -> Result<bool, RepoFileReadError>
where
    F: FnOnce(),
{
    match write_repo_file_atomic_impl_with_hook(
        root,
        relative_path,
        replacement,
        false,
        Some(expected),
        before_quarantine,
    ) {
        Ok(_) => Ok(true),
        Err(RepoFileReadError::ConcurrentModification) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub(crate) fn restore_repo_file_if_matches(
    root: &Path,
    relative_path: impl AsRef<Path>,
    expected: &RepoFileSnapshot,
    replacement: &[u8],
) -> Result<bool, RepoFileReadError> {
    match write_repo_file_atomic_impl(root, relative_path, replacement, false, Some(expected)) {
        Ok(_) => Ok(true),
        Err(RepoFileReadError::ConcurrentModification) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
pub(crate) fn remove_created_repo_directory(
    root: &Path,
    created: &RepoCreatedDirectory,
) -> Result<bool, RepoFileReadError> {
    use rustix::fs::{AtFlags, Mode, OFlags};

    let parent = open_repo_parent(root, &created.relative_path, false)?;
    let (transaction_parent, quarantine_name) = match quarantine_repo_target(&parent, || {}) {
        Ok(quarantine) => quarantine,
        Err(RepoFileReadError::NotFound) => return Ok(false),
        Err(error) => return Err(error),
    };
    let quarantined = match rustix::fs::openat(
        &transaction_parent,
        &quarantine_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fs::File::from(fd),
        Err(_) => {
            let _ = restore_directory_quarantine_noclobber(
                &transaction_parent,
                &quarantine_name,
                &parent.target_name,
            )?;
            return Ok(false);
        }
    };
    let identity = metadata_identity(
        &quarantined
            .metadata()
            .map_err(|_| RepoFileReadError::Metadata)?,
    );
    if identity != created.identity {
        let _ = restore_directory_quarantine_noclobber(
            &transaction_parent,
            &quarantine_name,
            &parent.target_name,
        )?;
        return Ok(false);
    }

    let racer_present = rustix::fs::statat(
        &transaction_parent,
        &parent.target_name,
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .is_ok();
    match rustix::fs::unlinkat(&transaction_parent, &quarantine_name, AtFlags::REMOVEDIR) {
        Ok(()) => Ok(!racer_present),
        Err(error) if error == rustix::io::Errno::NOTEMPTY => {
            let _ = restore_directory_quarantine_noclobber(
                &transaction_parent,
                &quarantine_name,
                &parent.target_name,
            )?;
            Ok(false)
        }
        Err(_) => Err(RepoFileReadError::Write),
    }
}

#[cfg(not(unix))]
pub(crate) fn remove_created_repo_directory(
    root: &Path,
    created: &RepoCreatedDirectory,
) -> Result<bool, RepoFileReadError> {
    let target = repo_write_target(root, &created.relative_path)?;
    let metadata = match fs::symlink_metadata(&target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(RepoFileReadError::Metadata),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata_identity(&metadata) != created.identity
    {
        return Ok(false);
    }
    match fs::remove_dir(target) {
        Ok(()) => Ok(true),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(false)
        }
        Err(_) => Err(RepoFileReadError::Write),
    }
}

pub(crate) fn create_dir_all_no_symlink(path: &Path) -> Result<(), RepoFileReadError> {
    ensure_no_symlink_ancestors(path)?;
    fs::create_dir_all(path).map_err(|_| RepoFileReadError::CreateDir)?;
    ensure_no_symlink_ancestors(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| RepoFileReadError::Metadata)?;
    if metadata.file_type().is_symlink() {
        return Err(RepoFileReadError::EscapesRepo);
    }
    if !metadata.is_dir() {
        return Err(RepoFileReadError::NotDirectory);
    }
    Ok(())
}

pub(crate) fn write_file_atomic_no_symlink(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> Result<(), RepoFileReadError> {
    let parent = path.parent().ok_or(RepoFileReadError::NotDirectory)?;
    create_dir_all_no_symlink(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.is_dir()
    {
        return Err(RepoFileReadError::NotFile);
    }
    write_file_atomic_in_existing_dir(parent, path, contents, false, None).map(|_| ())
}

pub(crate) fn managed_existing_file(
    root: &Path,
    managed_dir: &Path,
    path: &Path,
) -> Option<PathBuf> {
    ensure_managed_path(root, managed_dir).ok()?;
    ensure_managed_path(root, path).ok()?;
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }
    let canonical_root = root.canonicalize().ok()?;
    let canonical_managed_dir = managed_dir.canonicalize().ok()?;
    let canonical_path = path.canonicalize().ok()?;
    if canonical_managed_dir.starts_with(&canonical_root)
        && canonical_path.starts_with(&canonical_managed_dir)
    {
        Some(canonical_path)
    } else {
        None
    }
}

pub(crate) fn ensure_managed_path(root: &Path, path: &Path) -> Result<PathBuf, RepoFileReadError> {
    let root = normalize_path(root).ok_or(RepoFileReadError::EscapesRepo)?;
    let path = normalize_path(path).ok_or(RepoFileReadError::EscapesRepo)?;
    let relative = path
        .strip_prefix(&root)
        .map_err(|_| RepoFileReadError::EscapesRepo)?;
    create_dir_all_no_symlink(&root)?;
    let canonical_root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let mut current = root;
    let component_count = relative.components().count();

    for (index, component) in relative.components().enumerate() {
        let Component::Normal(segment) = component else {
            return Err(RepoFileReadError::EscapesRepo);
        };
        current.push(segment);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoFileReadError::EscapesRepo);
            }
            Ok(metadata) if index + 1 < component_count && !metadata.is_dir() => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Ok(_) => {
                let canonical_current = current
                    .canonicalize()
                    .map_err(|_| RepoFileReadError::Metadata)?;
                if !canonical_current.starts_with(&canonical_root) {
                    return Err(RepoFileReadError::EscapesRepo);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Err(_) => return Err(RepoFileReadError::Metadata),
        }
    }

    Ok(current)
}

pub(crate) fn ensure_no_symlink_ancestors(path: &Path) -> Result<(), RepoFileReadError> {
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoFileReadError::EscapesRepo);
            }
            Ok(metadata) => {
                if !missing.is_empty() && !metadata.is_dir() {
                    return Err(RepoFileReadError::NotDirectory);
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(name) = existing.file_name().map(ToOwned::to_owned) {
                    missing.push(name);
                }
                if !existing.pop() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Err(_) => return Err(RepoFileReadError::Metadata),
        }
    }
    for (index, name) in missing.iter().rev().enumerate() {
        existing.push(name);
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepoFileReadError::EscapesRepo);
            }
            Ok(metadata) if !metadata.is_dir() && index + 1 < missing.len() => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotADirectory => {
                return Err(RepoFileReadError::NotDirectory);
            }
            Err(_) => return Err(RepoFileReadError::Metadata),
        }
    }
    Ok(())
}

fn parent_identity(parent: &Path) -> Result<RepoFileIdentity, RepoFileReadError> {
    let metadata = fs::symlink_metadata(parent).map_err(|_| RepoFileReadError::Metadata)?;
    if metadata.file_type().is_symlink() {
        return Err(RepoFileReadError::EscapesRepo);
    }
    if !metadata.is_dir() {
        return Err(RepoFileReadError::NotDirectory);
    }
    Ok(metadata_identity(&metadata))
}

fn ensure_parent_identity(
    parent: &Path,
    expected: &RepoFileIdentity,
) -> Result<(), RepoFileReadError> {
    if parent_identity(parent)? == *expected {
        Ok(())
    } else {
        Err(RepoFileReadError::EscapesRepo)
    }
}

fn write_file_atomic_in_existing_dir(
    parent: &Path,
    target: &Path,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
) -> Result<RepoFileIdentity, RepoFileReadError> {
    write_file_atomic_in_existing_dir_with_hook(
        parent,
        target,
        contents,
        noclobber,
        expected,
        || {},
    )
}

#[cfg(unix)]
fn file_snapshot_at(
    parent: &fs::File,
    name: &std::ffi::OsStr,
) -> Result<RepoFileSnapshot, RepoFileReadError> {
    use rustix::fs::{Mode, OFlags};

    let fd = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::NOENT {
            RepoFileReadError::NotFound
        } else if error == rustix::io::Errno::LOOP {
            RepoFileReadError::EscapesRepo
        } else {
            RepoFileReadError::Read
        }
    })?;
    let mut file = fs::File::from(fd);
    let metadata = file.metadata().map_err(|_| RepoFileReadError::Metadata)?;
    if !metadata.is_file() {
        return Err(RepoFileReadError::NotFile);
    }
    let identity = metadata_identity(&metadata);
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|_| RepoFileReadError::Read)?;
    Ok(RepoFileSnapshot { identity, contents })
}

#[cfg(unix)]
fn ensure_repo_target_is_file_or_missing(parent: &RepoParentDir) -> Result<(), RepoFileReadError> {
    use rustix::fs::{AtFlags, FileType, statat};

    match statat(
        &parent.parent,
        &parent.target_name,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat) => match FileType::from_raw_mode(stat.st_mode) {
            FileType::Symlink => Err(RepoFileReadError::EscapesRepo),
            FileType::Directory => Err(RepoFileReadError::NotFile),
            _ => Ok(()),
        },
        Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
        Err(error) if error == rustix::io::Errno::NOTDIR => Err(RepoFileReadError::NotDirectory),
        Err(_) => Err(RepoFileReadError::Metadata),
    }
}

#[cfg(unix)]
fn write_repo_file_atomic_at_with_hook<F>(
    parent: &RepoParentDir,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    before_commit: F,
) -> Result<RepoFileIdentity, RepoFileReadError>
where
    F: FnOnce(),
{
    use std::sync::atomic::{AtomicU64, Ordering};

    use rustix::fs::{AtFlags, Mode, OFlags};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    ensure_repo_parent_rooted(parent)?;
    let (temp_name, temp_fd) = (0..128)
        .find_map(|_| {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let name = format!(".polint-tmp-{}-{suffix}", std::process::id());
            match rustix::fs::openat(
                &parent.parent,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => Some(Ok((name, fd))),
                Err(error) if error == rustix::io::Errno::EXIST => None,
                Err(_) => Some(Err(RepoFileReadError::Write)),
            }
        })
        .unwrap_or(Err(RepoFileReadError::Write))?;
    let mut temp_file = fs::File::from(temp_fd);

    let write_result = (|| {
        temp_file
            .write_all(contents.as_ref())
            .map_err(|_| RepoFileReadError::Write)?;
        temp_file.flush().map_err(|_| RepoFileReadError::Write)?;
        let committed_identity = metadata_identity(
            &temp_file
                .metadata()
                .map_err(|_| RepoFileReadError::Metadata)?,
        );

        before_commit();
        let transaction_parent = rooted_repo_parent(parent)?;

        if let Some(expected) = expected {
            let (transaction_parent, quarantine_name) = match quarantine_repo_target(parent, || {})
            {
                Ok(quarantine) => quarantine,
                Err(RepoFileReadError::NotFound) => {
                    return Err(RepoFileReadError::ConcurrentModification);
                }
                Err(error) => return Err(error),
            };

            match file_snapshot_at(&transaction_parent, &quarantine_name) {
                Ok(quarantined) if quarantined == *expected => {}
                Ok(_) | Err(RepoFileReadError::NotFile | RepoFileReadError::EscapesRepo) => {
                    restore_quarantine_noclobber(
                        &transaction_parent,
                        &quarantine_name,
                        &parent.target_name,
                    )?;
                    return Err(RepoFileReadError::ConcurrentModification);
                }
                Err(error) => {
                    if restore_quarantine_noclobber(
                        &transaction_parent,
                        &quarantine_name,
                        &parent.target_name,
                    )? {
                        return Err(error);
                    }
                    return Err(RepoFileReadError::ConcurrentModification);
                }
            }

            match rustix::fs::linkat(
                &transaction_parent,
                temp_name.as_str(),
                &transaction_parent,
                &parent.target_name,
                AtFlags::empty(),
            ) {
                Ok(()) => {
                    let _ = rustix::fs::unlinkat(
                        &transaction_parent,
                        temp_name.as_str(),
                        AtFlags::empty(),
                    );
                    let _ = unlink_quarantine(&transaction_parent, &quarantine_name);
                    return Ok(committed_identity);
                }
                Err(error) if error == rustix::io::Errno::EXIST => {
                    return Err(RepoFileReadError::ConcurrentModification);
                }
                Err(_) => {
                    return if restore_quarantine_noclobber(
                        &transaction_parent,
                        &quarantine_name,
                        &parent.target_name,
                    )? {
                        Err(RepoFileReadError::Persist)
                    } else {
                        Err(RepoFileReadError::ConcurrentModification)
                    };
                }
            }
        }

        if noclobber {
            rustix::fs::linkat(
                &transaction_parent,
                temp_name.as_str(),
                &transaction_parent,
                &parent.target_name,
                AtFlags::empty(),
            )
            .map_err(|_| RepoFileReadError::Persist)?;
            let _ = rustix::fs::unlinkat(&transaction_parent, temp_name.as_str(), AtFlags::empty());
        } else {
            rustix::fs::renameat(
                &transaction_parent,
                temp_name.as_str(),
                &transaction_parent,
                &parent.target_name,
            )
            .map_err(|_| RepoFileReadError::Persist)?;
        }
        Ok(committed_identity)
    })();

    if write_result.is_err() {
        let _ = rustix::fs::unlinkat(&parent.parent, temp_name.as_str(), AtFlags::empty());
    }
    write_result
}

#[cfg(unix)]
fn write_file_atomic_in_existing_dir_with_hook<F>(
    parent: &Path,
    target: &Path,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    after_temp_created: F,
) -> Result<RepoFileIdentity, RepoFileReadError>
where
    F: FnOnce(),
{
    use std::sync::atomic::{AtomicU64, Ordering};

    use rustix::fs::{AtFlags, Mode, OFlags};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    let expected_parent_identity = parent_identity(parent)?;
    let parent_fd = rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| RepoFileReadError::Metadata)?;
    let parent_file = fs::File::from(parent_fd);
    if metadata_identity(
        &parent_file
            .metadata()
            .map_err(|_| RepoFileReadError::Metadata)?,
    ) != expected_parent_identity
    {
        return Err(RepoFileReadError::EscapesRepo);
    }
    let target_name = target.file_name().ok_or(RepoFileReadError::NotFile)?;
    let (temp_name, temp_fd) = (0..128)
        .find_map(|_| {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let name = format!(".polint-tmp-{}-{suffix}", std::process::id());
            match rustix::fs::openat(
                &parent_file,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => Some(Ok((name, fd))),
                Err(error) if error == rustix::io::Errno::EXIST => None,
                Err(_) => Some(Err(RepoFileReadError::Write)),
            }
        })
        .unwrap_or(Err(RepoFileReadError::Write))?;
    let mut temp_file = fs::File::from(temp_fd);

    after_temp_created();
    let write_result = (|| {
        ensure_parent_identity(parent, &expected_parent_identity)?;
        temp_file
            .write_all(contents.as_ref())
            .map_err(|_| RepoFileReadError::Write)?;
        temp_file.flush().map_err(|_| RepoFileReadError::Write)?;
        if let Some(expected) = expected {
            match file_snapshot_at(&parent_file, target_name) {
                Ok(current) if current == *expected => {}
                Ok(_) | Err(RepoFileReadError::NotFound) => {
                    return Err(RepoFileReadError::ConcurrentModification);
                }
                Err(error) => return Err(error),
            }
        }
        let committed_identity = metadata_identity(
            &temp_file
                .metadata()
                .map_err(|_| RepoFileReadError::Metadata)?,
        );
        ensure_parent_identity(parent, &expected_parent_identity)?;
        if noclobber {
            rustix::fs::linkat(
                &parent_file,
                temp_name.as_str(),
                &parent_file,
                target_name,
                AtFlags::empty(),
            )
            .map_err(|_| RepoFileReadError::Persist)?;
            let _ = rustix::fs::unlinkat(&parent_file, temp_name.as_str(), AtFlags::empty());
        } else {
            rustix::fs::renameat(&parent_file, temp_name.as_str(), &parent_file, target_name)
                .map_err(|_| RepoFileReadError::Persist)?;
        }
        Ok(committed_identity)
    })();

    if write_result.is_err() {
        let _ = rustix::fs::unlinkat(&parent_file, temp_name.as_str(), AtFlags::empty());
    }
    write_result
}

#[cfg(not(unix))]
fn write_file_atomic_in_existing_dir_with_hook<F>(
    parent: &Path,
    target: &Path,
    contents: impl AsRef<[u8]>,
    noclobber: bool,
    expected: Option<&RepoFileSnapshot>,
    after_temp_created: F,
) -> Result<RepoFileIdentity, RepoFileReadError>
where
    F: FnOnce(),
{
    let parent_identity = parent_identity(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| RepoFileReadError::Write)?;
    after_temp_created();
    ensure_parent_identity(parent, &parent_identity)?;
    file.write_all(contents.as_ref())
        .map_err(|_| RepoFileReadError::Write)?;
    file.flush().map_err(|_| RepoFileReadError::Write)?;
    ensure_parent_identity(parent, &parent_identity)?;
    if let Some(expected) = expected {
        match file_snapshot(target) {
            Ok(current) if current == *expected => {}
            Ok(_) | Err(RepoFileReadError::NotFound) => {
                return Err(RepoFileReadError::ConcurrentModification);
            }
            Err(error) => return Err(error),
        }
    }
    let committed_identity = metadata_identity(
        &file
            .as_file()
            .metadata()
            .map_err(|_| RepoFileReadError::Metadata)?,
    );
    ensure_parent_identity(parent, &parent_identity)?;
    let result = if noclobber {
        file.persist_noclobber(target).map(|_| ())
    } else {
        file.persist(target).map(|_| ())
    };
    result.map_err(|_| RepoFileReadError::Persist)?;
    Ok(committed_identity)
}

fn canonical_repo_target(
    root: &Path,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, RepoFileReadError> {
    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    let root = root
        .canonicalize()
        .map_err(|_| RepoFileReadError::RootUnavailable)?;
    let joined = root.join(relative_path);
    let target = joined.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RepoFileReadError::NotFound
        } else {
            RepoFileReadError::Metadata
        }
    })?;
    if !target.starts_with(&root) {
        return Err(RepoFileReadError::EscapesRepo);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_repo_file_with_limit_rejects_oversized_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("package.json"), "{}\n").expect("write manifest");

        let error =
            read_repo_file_to_string_with_limit(temp.path(), "package.json", 2).unwrap_err();

        assert!(matches!(
            error,
            RepoFileReadError::TooLarge { max_bytes: 2 }
        ));
    }

    #[test]
    fn read_repo_file_rejects_absolute_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let error = read_repo_file(temp.path(), temp.path().join("package.json")).unwrap_err();

        assert!(matches!(error, RepoFileReadError::AbsolutePath));
    }

    #[cfg(unix)]
    #[test]
    fn read_repo_file_rejects_symlink_escape() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("package.json"), r#"{"name":"outside"}"#)
            .expect("write outside manifest");
        std::os::unix::fs::symlink(
            outside.path().join("package.json"),
            repo.path().join("package.json"),
        )
        .expect("symlink package.json");

        let error = read_repo_file_to_string_with_limit(repo.path(), "package.json", 1_048_576)
            .unwrap_err();

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
    }

    #[cfg(unix)]
    #[test]
    fn write_repo_file_atomic_rejects_symlink_parent_escape() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".polint"))
            .expect("symlink .polint");

        let error = write_repo_file_atomic(repo.path(), ".polint/output/latest.json", "{}")
            .expect_err("symlink parent should be rejected");

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
        assert!(!outside.path().join("output/latest.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn repo_write_target_rejects_symlinked_missing_parent_escape() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::create_dir_all(repo.path().join(".polint/rules")).expect("rules parent");
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".polint/rules/src"))
            .expect("symlink src");

        let error = repo_write_target(repo.path(), ".polint/rules/src/new_rule.rs")
            .expect_err("symlink parent should be rejected before creation");

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
        assert!(
            std::fs::read_dir(outside.path())
                .expect("outside")
                .next()
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_repo_file_atomic_noclobber_does_not_follow_existing_symlink() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::create_dir_all(repo.path().join(".polint/rules/src")).expect("rules src");
        let outside_file = outside.path().join("sentinel.rs");
        std::fs::write(&outside_file, "sentinel").expect("outside sentinel");
        std::os::unix::fs::symlink(&outside_file, repo.path().join(".polint/rules/src/demo.rs"))
            .expect("symlink destination");

        write_repo_file_atomic_noclobber(repo.path(), ".polint/rules/src/demo.rs", "replacement")
            .expect_err("noclobber write must reject an existing symlink");

        assert_eq!(
            std::fs::read_to_string(outside_file).expect("outside sentinel"),
            "sentinel"
        );
    }

    #[test]
    fn tracked_write_receipt_identifies_the_committed_file() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(".polint/rules/src")).expect("parent");
        std::fs::write(repo.path().join(".polint/rules/src/demo.rs"), "before").expect("original");
        let original = read_optional_repo_file_snapshot(repo.path(), ".polint/rules/src/demo.rs")
            .expect("snapshot")
            .expect("original exists");

        let receipt = write_repo_file_atomic_tracked(
            repo.path(),
            ".polint/rules/src/demo.rs",
            "after",
            &original,
            &mut Vec::new(),
        )
        .expect("tracked write");
        let committed = read_optional_repo_file_snapshot(repo.path(), ".polint/rules/src/demo.rs")
            .expect("committed snapshot")
            .expect("committed file exists");

        assert_eq!(receipt, committed.identity);
        assert_eq!(committed.contents, b"after");
    }

    #[cfg(unix)]
    #[test]
    fn repository_write_rejects_ancestor_moved_outside_with_symlink_back() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let ancestor = repo.path().join(".polint");
        let parent = ancestor.join("rules/src");
        std::fs::create_dir_all(&parent).expect("parent");
        let moved_ancestor = outside.path().join("moved-polint");

        let error = write_repo_file_atomic_impl_with_hook(
            repo.path(),
            ".polint/rules/src/demo.rs",
            b"transaction bytes",
            false,
            None,
            || {
                std::fs::rename(&ancestor, &moved_ancestor).expect("move repository ancestor");
                std::os::unix::fs::symlink(&moved_ancestor, &ancestor)
                    .expect("symlink moved ancestor back into repository");
            },
        )
        .expect_err("moved ancestor must no longer be rooted below the repository fd");

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
        assert!(!moved_ancestor.join("rules/src/demo.rs").exists());
        assert!(
            std::fs::read_dir(moved_ancestor.join("rules/src"))
                .expect("moved parent")
                .next()
                .is_none(),
            "temporary file must be removed through the held parent fd"
        );
    }

    #[cfg(unix)]
    #[test]
    fn created_directory_receipts_exclude_existing_directories_and_verify_identity() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir(repo.path().join(".polint")).expect("existing directory");
        let relative = ".polint/rules/src/demo.rs";
        let mut created_directories = Vec::new();
        write_repo_file_atomic_noclobber_tracked(
            repo.path(),
            relative,
            b"transaction bytes",
            &mut created_directories,
        )
        .expect("tracked write");

        assert!(
            created_directories
                .iter()
                .all(|created| created.relative_path != Path::new(".polint"))
        );
        let created_src = created_directories
            .iter()
            .find(|created| created.relative_path == Path::new(".polint/rules/src"))
            .expect("src creation receipt")
            .clone();
        std::fs::remove_file(repo.path().join(relative)).expect("remove committed file");
        let src = repo.path().join(".polint/rules/src");
        let original_src = repo.path().join(".polint/rules/original-src");
        std::fs::rename(&src, &original_src).expect("move created directory");
        std::fs::create_dir(&src).expect("concurrent replacement directory");

        let removed = remove_created_repo_directory(repo.path(), &created_src)
            .expect("identity mismatch is handled");

        assert!(!removed);
        assert!(src.is_dir(), "concurrent directory survives");
        assert!(
            original_src.is_dir(),
            "original created directory remains available"
        );
    }

    #[cfg(unix)]
    #[test]
    fn remove_rollback_preserves_target_replaced_at_quarantine_boundary() {
        let repo = tempfile::tempdir().expect("repo");
        let relative = ".polint/rules/src/demo.rs";
        let target = repo.path().join(relative);
        std::fs::create_dir_all(target.parent().expect("target parent")).expect("parent");
        std::fs::write(&target, "transaction bytes").expect("transaction file");
        let expected = read_optional_repo_file_snapshot(repo.path(), relative)
            .expect("snapshot")
            .expect("target exists");

        let removed =
            remove_repo_file_if_matches_with_hook(repo.path(), relative, &expected, || {
                let replacement = target.with_extension("replacement");
                std::fs::write(&replacement, "concurrent replacement").expect("write replacement");
                std::fs::rename(&replacement, &target).expect("publish replacement");
            })
            .expect("mismatched quarantine should be restored");

        assert!(!removed);
        assert_eq!(
            std::fs::read_to_string(target).expect("replacement survives"),
            "concurrent replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn restore_rollback_preserves_target_replaced_at_quarantine_boundary() {
        let repo = tempfile::tempdir().expect("repo");
        let relative = ".polint/rules/src/demo.rs";
        let target = repo.path().join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).expect("parent");
        std::fs::write(&target, "transaction bytes").expect("transaction file");
        let expected = read_optional_repo_file_snapshot(repo.path(), relative)
            .unwrap()
            .unwrap();

        let restored = restore_repo_file_if_matches_with_hook(
            repo.path(),
            relative,
            &expected,
            b"previous bytes",
            || {
                let replacement = target.with_extension("replacement");
                std::fs::write(&replacement, "concurrent replacement").expect("write replacement");
                std::fs::rename(&replacement, &target).expect("publish replacement");
            },
        )
        .expect("mismatched quarantine should be restored");

        assert!(!restored);
        assert_eq!(
            std::fs::read_to_string(target).unwrap(),
            "concurrent replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_rollback_rejects_ancestor_moved_outside_with_symlink_back() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let ancestor = repo.path().join(".polint");
        let target = ancestor.join("rules/src/demo.rs");
        std::fs::create_dir_all(target.parent().expect("target parent")).expect("parent");
        std::fs::write(&target, "tracked bytes").expect("tracked file");
        let expected = read_optional_repo_file_snapshot(repo.path(), ".polint/rules/src/demo.rs")
            .expect("snapshot")
            .expect("existing snapshot");
        let moved_ancestor = outside.path().join("moved-polint");

        let error = remove_repo_file_if_matches_with_hook(
            repo.path(),
            ".polint/rules/src/demo.rs",
            &expected,
            || {
                std::fs::rename(&ancestor, &moved_ancestor).expect("move repository ancestor");
                std::os::unix::fs::symlink(&moved_ancestor, &ancestor)
                    .expect("symlink moved ancestor back into repository");
            },
        )
        .expect_err("rollback must not unlink through a moved ancestor");

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
        assert_eq!(
            std::fs::read_to_string(moved_ancestor.join("rules/src/demo.rs"))
                .expect("tracked file remains"),
            "tracked bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_repo_write_refuses_moved_ancestor_symlinked_back_to_same_tree() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::create_dir_all(repo.path().join(".polint/rules/src")).expect("parents");
        let moved = outside.path().join("moved-polint");

        let error = write_repo_file_atomic_impl_with_hook(
            repo.path(),
            ".polint/rules/src/demo.rs",
            b"transaction bytes",
            false,
            None,
            || {
                std::fs::rename(repo.path().join(".polint"), &moved).expect("move ancestor");
                std::os::unix::fs::symlink(&moved, repo.path().join(".polint"))
                    .expect("symlink moved tree back");
            },
        )
        .expect_err("root fd walk must reject a replaced ancestor");

        assert!(
            matches!(error, RepoFileReadError::EscapesRepo),
            "unexpected error: {error:?}"
        );
        assert!(!moved.join("rules/src/demo.rs").exists());
    }

    /// Every per-file cache write goes through `ensure_repo_dir`, and the
    /// analysis pipeline writes them from a rayon pool. Losing the race to
    /// create a shared managed directory must not fail the write: before this
    /// was tolerated, a cold parallel run emitted one spurious
    /// `internal/cache` "cache write failed: create directory failed" warning
    /// per thread that lost.
    #[test]
    fn ensure_repo_dir_tolerates_concurrent_creation_of_the_same_directory() {
        let repo = tempfile::tempdir().expect("repo");
        // Several rounds over distinct fresh paths: the race only exists while
        // a component is still missing, so one round is one chance to hit it.
        for round in 0..8 {
            let relative = format!(".polint/cache/round_{round}/analysis");
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
            let failures = std::thread::scope(|scope| {
                // Every thread is spawned before any is joined, so they are all
                // parked on the barrier and race the same missing component.
                let mut handles = Vec::new();
                for _ in 0..16 {
                    let barrier = std::sync::Arc::clone(&barrier);
                    let relative = relative.clone();
                    let root = repo.path();
                    handles.push(scope.spawn(move || {
                        barrier.wait();
                        ensure_repo_dir(root, &relative)
                    }));
                }
                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().expect("thread").err())
                    .collect::<Vec<_>>()
            });
            assert!(
                failures.is_empty(),
                "round {round}: concurrent writers must all succeed, got {failures:?}"
            );
            assert!(repo.path().join(&relative).is_dir());
        }
    }

    /// The race tolerance must not weaken the escape guarantee: a component
    /// that already exists as a symlink is still rejected rather than written
    /// through.
    #[cfg(unix)]
    #[test]
    fn ensure_repo_dir_still_rejects_an_existing_symlink_component() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".polint"))
            .expect("symlink .polint");

        let error = ensure_repo_dir(repo.path(), ".polint/cache/analysis")
            .expect_err("symlink component should be rejected");

        assert!(matches!(error, RepoFileReadError::EscapesRepo));
        assert!(!outside.path().join("cache/analysis").exists());
    }

    #[test]
    fn ensure_no_symlink_ancestors_rejects_missing_path_below_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("cache"), "not a directory").expect("write file");

        let error = ensure_no_symlink_ancestors(
            &temp.path().join("cache").join("layers").join("blob.json"),
        )
        .expect_err("file ancestor should be rejected");

        assert!(matches!(error, RepoFileReadError::NotDirectory));
    }
}
