use std::path::{Component, Path, PathBuf};

pub(crate) const TOPOLOGY_MANIFEST_MAX_BYTES: u64 = 1_048_576;
pub(crate) const TOPOLOGY_LOCKFILE_MAX_BYTES: u64 = 16 * 1_048_576;

#[derive(Debug)]
pub(crate) enum RepoFileReadError {
    AbsolutePath,
    EscapesRepo,
    RootUnavailable,
    NotFound,
    NotFile,
    NotDirectory,
    Metadata,
    TooLarge { max_bytes: u64 },
    InvalidUtf8,
    Read,
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
    let metadata = std::fs::metadata(&target).map_err(|_| RepoFileReadError::Metadata)?;
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
    let metadata = std::fs::metadata(&target).map_err(|_| RepoFileReadError::Metadata)?;
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
    std::fs::read(target).map_err(|_| RepoFileReadError::Read)
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
    let metadata = std::fs::metadata(&target).map_err(|_| RepoFileReadError::Metadata)?;
    if metadata.len() > max_bytes {
        return Err(RepoFileReadError::TooLarge { max_bytes });
    }
    std::fs::read(target).map_err(|_| RepoFileReadError::Read)
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
