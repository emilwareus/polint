use std::path::{Component, Path, PathBuf};

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

pub(crate) fn normalize_repo_relative_path(root: &Path, path: &Path) -> Option<String> {
    let root = normalize_path(root)?;
    let path = normalize_path(path)?;
    let relative = path.strip_prefix(root).ok()?;
    normalize_repo_relative(relative.to_string_lossy())
}
