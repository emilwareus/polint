use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::manifest::{ExtensionActivationStatus, ExtensionManifest};
use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::module_graph::paths::{
    TOPOLOGY_LOCKFILE_MAX_BYTES, TOPOLOGY_MANIFEST_MAX_BYTES, normalize_repo_relative_input,
    read_repo_file_with_limit,
};

const EXTENSIONS_DIR: &str = ".polint/extensions";
const RUST_SOURCE_MAX_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DiscoveredExtension {
    pub(crate) extension_id: String,
    pub(crate) manifest_path: String,
    pub(crate) activation_status: ExtensionActivationStatus,
    pub(crate) manifest: ExtensionManifest,
    pub(crate) source_digest: Digest,
    pub(crate) dependency_digest: Digest,
    pub(crate) digest_input_paths: Vec<String>,
}

pub(crate) fn discover_local_extensions(root: &Path) -> Vec<DiscoveredExtension> {
    let extensions_root = root.join(EXTENSIONS_DIR);
    let Ok(entries) = fs::read_dir(&extensions_root) else {
        return Vec::new();
    };

    let mut manifests = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_real_dir(path))
        .filter_map(|path| discovered_extension(root, &path))
        .collect::<Vec<_>>();
    manifests.sort_by(|left, right| left.manifest_path.cmp(&right.manifest_path));
    manifests
}

fn discovered_extension(root: &Path, extension_dir: &Path) -> Option<DiscoveredExtension> {
    let manifest_path = extension_dir.join("Cargo.toml");
    if !is_real_file(&manifest_path) {
        return None;
    }

    let extension_id = extension_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)?;
    let manifest = ExtensionManifest::repo_local(extension_id.clone()).ok()?;
    let manifest_path = repo_relative_path(root, &manifest_path)?;
    let dependency_paths = dependency_digest_paths(root, extension_dir)?;
    let source_paths = source_digest_paths(root, extension_dir)?;
    let source_digest = digest_files(
        root,
        "extension_source",
        &source_paths,
        RUST_SOURCE_MAX_BYTES,
    );
    let dependency_digest = digest_files(
        root,
        "extension_dependency",
        &dependency_paths,
        TOPOLOGY_LOCKFILE_MAX_BYTES,
    );
    let mut digest_input_paths = dependency_paths;
    digest_input_paths.extend(source_paths);
    digest_input_paths.sort();
    digest_input_paths.dedup();

    Some(DiscoveredExtension {
        extension_id,
        manifest_path,
        activation_status: ExtensionActivationStatus::Discovered,
        manifest,
        source_digest,
        dependency_digest,
        digest_input_paths,
    })
}

fn dependency_digest_paths(root: &Path, extension_dir: &Path) -> Option<Vec<String>> {
    ["Cargo.toml", "Cargo.lock"]
        .into_iter()
        .map(|file_name| {
            let path = extension_dir.join(file_name);
            if !path.exists() {
                return Some(None);
            }
            if !is_real_file(&path) {
                return None;
            }
            Some(repo_relative_path(root, &path))
        })
        .collect::<Option<Vec<_>>>()
        .map(|paths| paths.into_iter().flatten().collect())
}

fn source_digest_paths(root: &Path, extension_dir: &Path) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    collect_rust_sources(root, &extension_dir.join("src"), &mut paths).then(|| {
        paths.sort();
        paths
    })
}

fn collect_rust_sources(root: &Path, dir: &Path, paths: &mut Vec<String>) -> bool {
    let Ok(metadata) = fs::symlink_metadata(dir) else {
        return true;
    };
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return false;
    }
    if !file_type.is_dir() {
        return true;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return true;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            return false;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return false;
        }
        if file_type.is_dir() {
            if !collect_rust_sources(root, &path, paths) {
                return false;
            }
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            && let Some(relative_path) = repo_relative_path(root, &path)
        {
            paths.push(relative_path);
        }
    }
    true
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn digest_files(root: &Path, label: &str, paths: &[String], default_max_bytes: u64) -> Digest {
    let mut parts = Vec::new();
    for path in paths {
        let max_bytes = if path.ends_with("Cargo.toml") {
            TOPOLOGY_MANIFEST_MAX_BYTES
        } else {
            default_max_bytes
        };
        parts.push(format!("path={path}"));
        match read_repo_file_with_limit(root, path, max_bytes) {
            Ok(contents) => {
                parts.push(format!("content_hash={}", stable_hash_bytes(&contents)));
            }
            Err(_error) => {
                parts.push("read_error=unreadable".to_string());
            }
        }
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ExtensionCode, label, &refs)
}

fn repo_relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    normalize_repo_relative_input(relative).map(|path| path.to_string_lossy().replace('\\', "/"))
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, contents).expect("write fixture file");
    }

    #[test]
    fn missing_extension_directory_returns_empty_discovery() {
        let temp = TempDir::new().expect("create temp repo");

        assert!(discover_local_extensions(temp.path()).is_empty());
    }

    #[test]
    fn extensions_are_discovered_in_deterministic_path_order() {
        let temp = TempDir::new().expect("create temp repo");
        write_file(
            temp.path(),
            ".polint/extensions/zeta/Cargo.toml",
            "[package]\nname = \"zeta\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write_file(
            temp.path(),
            ".polint/extensions/alpha/Cargo.toml",
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );

        let discovered = discover_local_extensions(temp.path());

        assert_eq!(
            discovered
                .iter()
                .map(|extension| extension.manifest_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                ".polint/extensions/alpha/Cargo.toml",
                ".polint/extensions/zeta/Cargo.toml"
            ]
        );
    }

    #[test]
    fn source_file_changes_change_extension_code_digest() {
        let temp = TempDir::new().expect("create temp repo");
        write_file(
            temp.path(),
            ".polint/extensions/demo/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write_file(
            temp.path(),
            ".polint/extensions/demo/src/main.rs",
            "fn main() {}\n",
        );
        let first = discover_local_extensions(temp.path())
            .pop()
            .expect("extension discovered");

        write_file(
            temp.path(),
            ".polint/extensions/demo/src/main.rs",
            "fn main() { println!(\"changed\"); }\n",
        );
        let second = discover_local_extensions(temp.path())
            .pop()
            .expect("extension discovered");

        assert_ne!(first.source_digest, second.source_digest);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_extension_directories_are_not_discovered() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside extension");
        fs::create_dir_all(temp.path().join(".polint/extensions")).expect("create extension root");
        write_file(
            outside.path(),
            "Cargo.toml",
            "[package]\nname = \"outside\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        symlink(
            outside.path(),
            temp.path().join(".polint/extensions/outside"),
        )
        .expect("create extension symlink");

        assert!(discover_local_extensions(temp.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_extension_sources_are_not_discovered() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside source");
        write_file(
            temp.path(),
            ".polint/extensions/demo/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write_file(outside.path(), "main.rs", "fn main() {}\n");
        fs::create_dir_all(temp.path().join(".polint/extensions/demo/src"))
            .expect("create extension src");
        symlink(
            outside.path().join("main.rs"),
            temp.path().join(".polint/extensions/demo/src/main.rs"),
        )
        .expect("create source symlink");

        assert!(discover_local_extensions(temp.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_extension_source_directories_are_not_discovered() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside source");
        write_file(
            temp.path(),
            ".polint/extensions/demo/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write_file(outside.path(), "main.rs", "fn main() {}\n");
        fs::create_dir_all(temp.path().join(".polint/extensions/demo"))
            .expect("create extension dir");
        symlink(
            outside.path(),
            temp.path().join(".polint/extensions/demo/src"),
        )
        .expect("create source dir symlink");

        assert!(discover_local_extensions(temp.path()).is_empty());
    }
}
