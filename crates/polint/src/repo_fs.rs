use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

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
            Self::CreateDir => "directory creation failed",
            Self::Write => "write failed",
            Self::Persist => "atomic persist failed",
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
                fs::create_dir(&current).map_err(|_| RepoFileReadError::CreateDir)?;
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
    ensure_no_symlink_ancestors(root).ok()?;
    ensure_no_symlink_ancestors(managed_dir).ok()?;
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
