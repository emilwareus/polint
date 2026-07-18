use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

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
#[path = "repo_fs/anchored_fallback.rs"]
mod anchored;
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd"
))]
#[path = "repo_fs/anchored_unix.rs"]
mod anchored;
#[cfg(windows)]
#[path = "repo_fs/anchored_windows.rs"]
mod anchored;

pub(crate) use anchored::{RepoDirectory, RepoDirectoryEntry, RepoDirectoryEntryKind, RepoFile};

pub(crate) const TOPOLOGY_MANIFEST_MAX_BYTES: u64 = 1_048_576;
pub(crate) const TOPOLOGY_LOCKFILE_MAX_BYTES: u64 = 16 * 1_048_576;

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
    #[error("filesystem resources exhausted")]
    ResourceExhausted,
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
    #[error("secure anchored file reads are unavailable on this platform")]
    SecureOpenUnavailable,
    #[error("create directory failed")]
    CreateDir,
    #[error("write failed")]
    Write,
    #[error("atomic persist failed")]
    Persist,
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
            Self::ResourceExhausted => "filesystem resources exhausted",
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
            Self::SecureOpenUnavailable => "secure anchored file reads unavailable",
            Self::CreateDir => "directory creation failed",
            Self::Write => "write failed",
            Self::Persist => "atomic persist failed",
        }
    }

    pub(crate) fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    pub(crate) fn is_resource_exhausted(&self) -> bool {
        matches!(self, Self::ResourceExhausted)
    }

    pub(crate) fn is_secure_open_unavailable(&self) -> bool {
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
        {
            return matches!(self, Self::SecureOpenUnavailable);
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
        false
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

#[cfg(test)]
pub(crate) fn read_repo_file_anchored_to_string_with_limit(
    root: &Path,
    relative_path: impl AsRef<Path>,
    max_bytes: u64,
) -> Result<String, RepoFileReadError> {
    let bytes = read_repo_file_anchored_with_limit(root, relative_path, max_bytes)?;
    String::from_utf8(bytes).map_err(|_| RepoFileReadError::InvalidUtf8)
}

#[cfg(test)]
fn read_repo_file_anchored_with_limit(
    root: &Path,
    relative_path: impl AsRef<Path>,
    max_bytes: u64,
) -> Result<Vec<u8>, RepoFileReadError> {
    let relative_path =
        normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
    let Some(file_name) = relative_path.file_name() else {
        return Err(RepoFileReadError::NotFile);
    };
    let parent = relative_path.parent().unwrap_or_else(|| Path::new("."));
    RepoDirectory::open(root, parent)?
        .open_file(file_name)?
        .read_with_limit(max_bytes)
}

fn read_open_file_with_limit(
    file: &mut fs::File,
    file_len: u64,
    max_bytes: u64,
) -> Result<Vec<u8>, RepoFileReadError> {
    if file_len > max_bytes {
        return Err(RepoFileReadError::TooLarge { max_bytes });
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(map_repo_file_read_error)?;
    if bytes.len() as u64 > max_bytes {
        return Err(RepoFileReadError::TooLarge { max_bytes });
    }
    Ok(bytes)
}

fn map_repo_file_read_error(error: std::io::Error) -> RepoFileReadError {
    if error.kind() == std::io::ErrorKind::OutOfMemory {
        return RepoFileReadError::ResourceExhausted;
    }
    #[cfg(unix)]
    if matches!(
        error.raw_os_error(),
        Some(libc::EMFILE) | Some(libc::ENFILE) | Some(libc::ENOMEM)
    ) {
        return RepoFileReadError::ResourceExhausted;
    }
    #[cfg(windows)]
    if matches!(
        error.raw_os_error().map(|code| code as u32),
        Some(windows_sys::Win32::Foundation::ERROR_TOO_MANY_OPEN_FILES)
            | Some(windows_sys::Win32::Foundation::ERROR_NOT_ENOUGH_MEMORY)
            | Some(windows_sys::Win32::Foundation::ERROR_OUTOFMEMORY)
            | Some(windows_sys::Win32::Foundation::ERROR_NO_SYSTEM_RESOURCES)
            | Some(windows_sys::Win32::Foundation::ERROR_WORKING_SET_QUOTA)
            | Some(windows_sys::Win32::Foundation::ERROR_NOT_ENOUGH_QUOTA)
    ) {
        return RepoFileReadError::ResourceExhausted;
    }
    RepoFileReadError::Read
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
                create_or_validate_repo_dir_component(&current)?;
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

fn create_or_validate_repo_dir_component(path: &Path) -> Result<(), RepoFileReadError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|_| RepoFileReadError::Metadata)?;
            if metadata.file_type().is_symlink() {
                Err(RepoFileReadError::EscapesRepo)
            } else if metadata.is_dir() {
                Ok(())
            } else {
                Err(RepoFileReadError::NotDirectory)
            }
        }
        Err(_) => Err(RepoFileReadError::CreateDir),
    }
}

pub(crate) fn write_repo_file_atomic(
    root: &Path,
    relative_path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> Result<(), RepoFileReadError> {
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
    if let Ok(metadata) = fs::symlink_metadata(&target)
        && metadata.is_dir()
    {
        return Err(RepoFileReadError::NotFile);
    }
    write_file_atomic_in_existing_dir(&parent, &target, contents)
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
    write_file_atomic_in_existing_dir(parent, path, contents)
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

fn write_file_atomic_in_existing_dir(
    parent: &Path,
    target: &Path,
    contents: impl AsRef<[u8]>,
) -> Result<(), RepoFileReadError> {
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| RepoFileReadError::Write)?;
    file.write_all(contents.as_ref())
        .map_err(|_| RepoFileReadError::Write)?;
    file.flush().map_err(|_| RepoFileReadError::Write)?;
    file.persist(target)
        .map(|_| ())
        .map_err(|_| RepoFileReadError::Persist)
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

pub(crate) fn normalize_repo_relative_path(root: &Path, path: &Path) -> Option<String> {
    let root = normalize_path(root)?;
    let path = normalize_path(path)?;
    let relative = path.strip_prefix(root).ok()?;
    normalize_repo_relative(relative.to_string_lossy())
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
    fn anchored_read_supports_regular_files_and_enforces_the_limit() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("nested")).expect("nested directory");
        std::fs::write(temp.path().join("nested/model.toml"), "schema = 1\n").expect("write model");

        assert_eq!(
            read_repo_file_anchored_to_string_with_limit(temp.path(), "nested/model.toml", 1_024,)
                .expect("anchored read"),
            "schema = 1\n"
        );
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(temp.path(), "nested/model.toml", 2),
            Err(RepoFileReadError::TooLarge { max_bytes: 2 })
        ));
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
    fn pinned_directory_enumeration_can_be_repeated() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("a.toml"), "a = 1\n").expect("write a");
        std::fs::write(temp.path().join("b.toml"), "b = 1\n").expect("write b");
        let directory = RepoDirectory::open(temp.path(), Path::new(".")).expect("open directory");

        let collect_names = || {
            let mut names = Vec::new();
            directory
                .visit_entries(|entry| {
                    names.push(entry.name);
                    true
                })
                .expect("enumerate directory");
            names.sort();
            names
        };

        assert_eq!(collect_names(), collect_names());
    }

    #[cfg(windows)]
    #[test]
    fn empty_pinned_directory_enumeration_can_be_repeated() {
        let temp = tempfile::tempdir().expect("tempdir");
        let directory = RepoDirectory::open(temp.path(), Path::new(".")).expect("open directory");

        for _ in 0..2 {
            let mut names = Vec::new();
            directory
                .visit_entries(|entry| {
                    names.push(entry.name);
                    true
                })
                .expect("enumerate empty directory");
            assert!(names.is_empty());
        }
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
    fn anchored_enumeration_classifies_socket_without_opening_it_as_content() {
        use std::os::unix::net::UnixListener;

        let temp = tempfile::tempdir().expect("tempdir");
        let _socket = UnixListener::bind(temp.path().join("service.sock")).expect("bind socket");
        let directory = RepoDirectory::open(temp.path(), Path::new(".")).expect("open directory");
        let mut saw_socket = false;

        directory
            .visit_entries(|entry| {
                if entry.name == "service.sock" {
                    saw_socket = matches!(entry.kind, Ok(RepoDirectoryEntryKind::Other));
                }
                true
            })
            .expect("enumerate directory");

        assert!(saw_socket);
    }

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
    fn read_resource_exhaustion_is_not_downgraded_to_a_per_file_error() {
        assert!(matches!(
            map_repo_file_read_error(std::io::Error::from(std::io::ErrorKind::OutOfMemory)),
            RepoFileReadError::ResourceExhausted
        ));
        #[cfg(unix)]
        assert!(matches!(
            map_repo_file_read_error(std::io::Error::from_raw_os_error(libc::ENOMEM)),
            RepoFileReadError::ResourceExhausted
        ));
        assert!(matches!(
            map_repo_file_read_error(std::io::Error::other("ordinary read failure")),
            RepoFileReadError::Read
        ));
    }

    #[test]
    fn read_repo_file_rejects_absolute_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let error = read_repo_file(temp.path(), temp.path().join("package.json")).unwrap_err();

        assert!(matches!(error, RepoFileReadError::AbsolutePath));
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
    fn anchored_read_rejects_root_intermediate_and_file_symlinks() {
        let outside = tempfile::tempdir().expect("outside");
        std::fs::create_dir_all(outside.path().join("nested")).expect("outside nested");
        std::fs::write(outside.path().join("nested/model.toml"), "secret = true\n")
            .expect("outside model");

        let container = tempfile::tempdir().expect("container");
        let root_link = container.path().join("root-link");
        std::os::unix::fs::symlink(outside.path(), &root_link).expect("root symlink");
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(&root_link, "nested/model.toml", 1_024,),
            Err(RepoFileReadError::EscapesRepo)
        ));

        let intermediate_repo = tempfile::tempdir().expect("intermediate repo");
        std::os::unix::fs::symlink(
            outside.path().join("nested"),
            intermediate_repo.path().join("nested"),
        )
        .expect("intermediate symlink");
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(
                intermediate_repo.path(),
                "nested/model.toml",
                1_024,
            ),
            Err(RepoFileReadError::EscapesRepo)
        ));

        let file_repo = tempfile::tempdir().expect("file repo");
        std::os::unix::fs::symlink(
            outside.path().join("nested/model.toml"),
            file_repo.path().join("model.toml"),
        )
        .expect("file symlink");
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(file_repo.path(), "model.toml", 1_024),
            Err(RepoFileReadError::EscapesRepo)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn anchored_read_rejects_root_intermediate_and_file_reparse_points() {
        use std::os::windows::fs::symlink_file;
        use std::process::Command;

        fn junction(target: &Path, link: &Path) {
            let status = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .status()
                .expect("run mklink");
            assert!(status.success(), "junction creation failed");
        }

        let outside = tempfile::tempdir().expect("outside");
        std::fs::create_dir_all(outside.path().join("nested")).expect("outside nested");
        std::fs::write(outside.path().join("nested/model.toml"), "secret = true\n")
            .expect("outside model");

        let container = tempfile::tempdir().expect("container");
        let root_link = container.path().join("root-link");
        junction(outside.path(), &root_link);
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(&root_link, "nested/model.toml", 1_024,),
            Err(RepoFileReadError::EscapesRepo)
        ));

        let intermediate_repo = tempfile::tempdir().expect("intermediate repo");
        junction(
            &outside.path().join("nested"),
            &intermediate_repo.path().join("nested"),
        );
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(
                intermediate_repo.path(),
                "nested/model.toml",
                1_024,
            ),
            Err(RepoFileReadError::EscapesRepo)
        ));

        let file_repo = tempfile::tempdir().expect("file repo");
        if let Err(error) = symlink_file(
            outside.path().join("nested/model.toml"),
            file_repo.path().join("model.toml"),
        ) {
            if error.raw_os_error() == Some(1_314) {
                return;
            }
            panic!("file symlink: {error}");
        }
        assert!(matches!(
            read_repo_file_anchored_to_string_with_limit(file_repo.path(), "model.toml", 1_024),
            Err(RepoFileReadError::EscapesRepo)
        ));
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

    #[test]
    fn concurrent_repo_writes_share_new_parent_directories() {
        let repo = tempfile::tempdir().expect("repo");
        let worker_count = 32;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(worker_count));
        let mut workers = Vec::new();
        for worker in 0..worker_count {
            let root = repo.path().to_path_buf();
            let barrier = std::sync::Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                let relative = format!(".polint/cache/analysis/{worker}.json");
                write_repo_file_atomic(&root, &relative, format!("{worker}\n"))
                    .unwrap_or_else(|error| panic!("worker {worker}: {error}"));
            }));
        }
        for worker in workers {
            worker.join().expect("cache writer completes");
        }
        for worker in 0..worker_count {
            assert_eq!(
                std::fs::read_to_string(
                    repo.path()
                        .join(format!(".polint/cache/analysis/{worker}.json"))
                )
                .expect("read cache entry"),
                format!("{worker}\n")
            );
        }
    }

    #[test]
    fn lost_directory_creation_race_revalidates_the_winner() {
        let repo = tempfile::tempdir().expect("repo");
        let directory = repo.path().join("cache");
        std::fs::create_dir(&directory).expect("simulate winning directory creator");

        create_or_validate_repo_dir_component(&directory)
            .expect("an existing real directory wins the creation race");

        let file = repo.path().join("not-a-directory");
        std::fs::write(&file, "hostile").expect("write hostile entry");
        assert!(matches!(
            create_or_validate_repo_dir_component(&file),
            Err(RepoFileReadError::NotDirectory)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn lost_directory_creation_race_rejects_a_symlink_winner() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        let link = repo.path().join("cache");
        std::os::unix::fs::symlink(outside.path(), &link).expect("create hostile symlink");

        assert!(matches!(
            create_or_validate_repo_dir_component(&link),
            Err(RepoFileReadError::EscapesRepo)
        ));
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
