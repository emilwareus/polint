use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) fn materialize_embedded_sources(
    cache_name: &str,
    version: &str,
    content_hash: &str,
    files: &[(&str, &str)],
) -> Result<PathBuf, String> {
    let root = private_cache_root()?;
    let sidecars = create_private_child(&root, "go-sidecars")?;
    let cache = create_private_child(&sidecars, cache_name)?;
    let parent = create_private_child(&cache, version)?;
    materialize_embedded_sources_at(&parent, content_hash, files)
}

fn private_cache_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("polint").join("cache"));
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join("Library").join("Caches").join("polint"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".cache"))
        })
        .map(|path| path.join("polint"));
    #[cfg(not(any(unix, target_os = "windows")))]
    let root: Option<PathBuf> = None;

    let root = root.ok_or_else(|| {
        "cannot locate a private per-user cache for embedded Go sidecars".to_string()
    })?;
    create_private_dir_all(&root)?;
    verify_private_dir(&root)?;
    Ok(root)
}

fn create_private_child(parent: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(std::path::MAIN_SEPARATOR)
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(format!(
            "invalid embedded Go sidecar cache component `{name}`"
        ));
    }
    verify_private_dir(parent)?;
    let child = parent.join(name);
    if !child.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(&child).map_err(|error| {
                format!(
                    "failed to create private cache `{}`: {error}",
                    child.display()
                )
            })?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&child).map_err(|error| {
            format!(
                "failed to create private cache `{}`: {error}",
                child.display()
            )
        })?;
    }
    verify_private_dir(&child)?;
    Ok(child)
}

fn materialize_embedded_sources_at(
    parent: &Path,
    content_hash: &str,
    files: &[(&str, &str)],
) -> Result<PathBuf, String> {
    create_private_dir_all(parent)?;
    verify_private_dir(parent)?;
    let directory = parent.join(content_hash);
    if verified_embedded_directory(&directory, content_hash, files) {
        return Ok(directory);
    }
    if directory.exists() {
        // Never invalidate a shared path in place: another process may already
        // be using the directory after its own successful verification.
        let fallback_parent = parent.join(format!(
            ".fallback-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        create_private_dir_all(&fallback_parent)?;
        return materialize_embedded_sources_at(&fallback_parent, content_hash, files);
    }

    let staging = parent.join(format!(
        ".{content_hash}-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    remove_cache_entry(&staging)?;
    create_private_dir_all(&staging)?;
    write_embedded_files(&staging, files)?;
    write_private_file(&staging.join(".complete"), content_hash.as_bytes())?;

    match fs::rename(&staging, &directory) {
        Ok(()) => {}
        Err(_error) if verified_embedded_directory(&directory, content_hash, files) => {
            let _ = fs::remove_dir_all(&staging);
            return Ok(directory);
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!(
                "failed to publish embedded Go sidecar cache `{}`: {error}",
                directory.display()
            ));
        }
    }
    if !verified_embedded_directory(&directory, content_hash, files) {
        return Err(format!(
            "published embedded Go sidecar cache `{}` failed content or ownership verification",
            directory.display()
        ));
    }
    Ok(directory)
}

pub(crate) fn verified_embedded_directory(
    directory: &Path,
    content_hash: &str,
    files: &[(&str, &str)],
) -> bool {
    verify_private_dir(directory).is_ok()
        && read_verified_private_file(&directory.join(".complete"))
            .is_ok_and(|contents| contents == content_hash.as_bytes())
        && files.iter().all(|(relative, expected)| {
            read_verified_private_file(&directory.join(relative))
                .is_ok_and(|contents| contents == expected.as_bytes())
        })
        && embedded_source_paths(directory).is_ok_and(|actual| {
            let expected = files
                .iter()
                .map(|(relative, _)| relative.replace('\\', "/"))
                .collect::<BTreeSet<_>>();
            actual == expected
        })
}

fn embedded_source_paths(directory: &Path) -> Result<BTreeSet<String>, String> {
    let mut paths = BTreeSet::new();
    collect_embedded_source_paths(directory, directory, &mut paths)?;
    Ok(paths)
}

fn collect_embedded_source_paths(
    root: &Path,
    directory: &Path,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to inspect `{}`: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect an entry in `{}`: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("failed to inspect `{}`: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "embedded Go sidecar cache contains symlink `{}`",
                path.display()
            ));
        }
        if metadata.is_dir() {
            verify_private_metadata(&path, &metadata, true)?;
            collect_embedded_source_paths(root, &path, paths)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if !is_embedded_runtime_artifact(&relative) {
                verify_private_metadata(&path, &metadata, false)?;
                paths.insert(relative);
            }
        } else {
            return Err(format!(
                "embedded Go sidecar cache contains unsupported entry `{}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn is_embedded_runtime_artifact(relative: &str) -> bool {
    !relative.contains('/')
        && (matches!(
            relative,
            ".complete" | "polint-go-frontend" | "polint-go-frontend.exe"
        ) || relative.starts_with(".build-")
            || relative.starts_with(".polint-go-frontend-")
            || relative.starts_with(".binary-")
            || relative.starts_with(".binary-lock-")
            || relative.starts_with(".binary-receipt-"))
}

fn write_embedded_files(directory: &Path, files: &[(&str, &str)]) -> Result<(), String> {
    for (relative, contents) in files {
        let path = directory.join(relative);
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent)?;
        }
        write_private_file(&path, contents.as_bytes())?;
    }
    Ok(())
}

pub(crate) fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        format!(
            "failed to create private cache file `{}`: {error}",
            path.display()
        )
    })?;
    file.write_all(contents).map_err(|error| {
        format!(
            "failed to write private cache file `{}`: {error}",
            path.display()
        )
    })?;
    file.sync_all().map_err(|error| {
        format!(
            "failed to sync private cache file `{}`: {error}",
            path.display()
        )
    })
}

pub(crate) fn read_verified_private_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect `{}`: {error}", path.display()))?;
    verify_private_metadata(path, &metadata, false)?;
    fs::read(path).map_err(|error| format!("failed to read `{}`: {error}", path.display()))
}

fn create_private_dir_all(path: &Path) -> Result<(), String> {
    if path.exists() {
        return verify_private_dir(path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(path).map_err(|error| {
            format!(
                "failed to create private cache `{}`: {error}",
                path.display()
            )
        })?;
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path).map_err(|error| {
        format!(
            "failed to create private cache `{}`: {error}",
            path.display()
        )
    })?;
    verify_private_dir(path)
}

fn verify_private_dir(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect private cache `{}`: {error}",
            path.display()
        )
    })?;
    verify_private_metadata(path, &metadata, true)
}

#[cfg(unix)]
fn verify_private_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    directory: bool,
) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    #[allow(unsafe_code)]
    // SAFETY: geteuid takes no arguments and has no preconditions.
    let effective_uid = unsafe { libc::geteuid() };
    let expected_kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if metadata.file_type().is_symlink()
        || !expected_kind
        || metadata.uid() != effective_uid
        || metadata.mode() & 0o077 != 0
    {
        return Err(format!(
            "embedded Go sidecar cache `{}` is not a private regular {} owned by the current user",
            path.display(),
            if directory { "directory" } else { "file" }
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_private_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    directory: bool,
) -> Result<(), String> {
    let expected_kind = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if metadata.file_type().is_symlink() || !expected_kind {
        return Err(format!(
            "embedded Go sidecar cache `{}` is not a regular {}",
            path.display(),
            if directory { "directory" } else { "file" }
        ));
    }
    Ok(())
}

fn remove_cache_entry(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
    .map_err(|error| {
        format!(
            "failed to remove untrusted cache entry `{}`: {error}",
            path.display()
        )
    })
}

fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[(&str, &str)] = &[("go.mod", "module test\n"), ("main.go", "package main\n")];

    #[test]
    fn preseeded_completion_marker_does_not_authorize_changed_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        let seeded = parent.join("expected");
        create_private_dir_all(&seeded).expect("create preseed");
        write_private_file(&seeded.join("go.mod"), b"module attacker\n").expect("seed go.mod");
        write_private_file(&seeded.join("main.go"), b"package main\n").expect("seed main");
        write_private_file(&seeded.join(".complete"), b"expected").expect("seed marker");

        let directory =
            materialize_embedded_sources_at(&parent, "expected", FILES).expect("replace preseed");
        assert_eq!(
            fs::read_to_string(directory.join("go.mod")).unwrap(),
            "module test\n"
        );
    }

    #[test]
    fn extra_go_source_invalidates_cache() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        let directory =
            materialize_embedded_sources_at(&parent, "expected", FILES).expect("materialize");
        write_private_file(&directory.join("attacker.go"), b"package main\n").expect("add source");

        let directory = materialize_embedded_sources_at(&parent, "expected", FILES)
            .expect("replace cache with extra source");
        assert!(!directory.join("attacker.go").exists());
    }

    #[test]
    fn every_unexpected_regular_file_invalidates_cache() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");

        for unexpected in ["attacker.s", "attacker.syso", "README.md"] {
            let directory =
                materialize_embedded_sources_at(&parent, "expected", FILES).expect("materialize");
            write_private_file(
                &directory.join(unexpected),
                b"attacker-controlled build input",
            )
            .expect("add unexpected file");
            assert!(!verified_embedded_directory(&directory, "expected", FILES));

            let rebuilt = materialize_embedded_sources_at(&parent, "expected", FILES)
                .expect("replace cache with unexpected file");
            assert!(!rebuilt.join(unexpected).exists());
        }
    }

    #[test]
    fn known_non_build_runtime_artifacts_do_not_change_source_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        let directory =
            materialize_embedded_sources_at(&parent, "expected", FILES).expect("materialize");
        for runtime_file in [
            ".binary-lock",
            ".binary-receipt",
            ".binary-receipt.receipt-1-2",
            ".build-1-2",
            "polint-go-frontend",
            "polint-go-frontend.exe",
        ] {
            write_private_file(&directory.join(runtime_file), b"runtime artifact")
                .expect("write runtime artifact");
        }

        assert!(verified_embedded_directory(&directory, "expected", FILES));
    }

    #[test]
    fn competing_publishers_accept_only_verified_content() {
        use std::sync::{Arc, Barrier};

        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        create_private_dir_all(&parent).expect("create parent");
        let barrier = Arc::new(Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let parent = parent.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    materialize_embedded_sources_at(&parent, "expected", FILES)
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            let directory = handle.join().expect("publisher thread").expect("publish");
            assert!(verified_embedded_directory(&directory, "expected", FILES));
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_preseed_is_replaced_without_touching_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        let outside = temp.path().join("outside");
        create_private_dir_all(&parent).expect("create parent");
        create_private_dir_all(&outside).expect("create outside");
        write_private_file(&outside.join("sentinel"), b"keep").expect("write sentinel");
        symlink(&outside, parent.join("expected")).expect("seed symlink");

        let directory =
            materialize_embedded_sources_at(&parent, "expected", FILES).expect("replace symlink");
        assert!(
            !fs::symlink_metadata(directory)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn permissive_cache_is_rejected_and_rebuilt_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let parent = temp.path().join("cache");
        let directory =
            materialize_embedded_sources_at(&parent, "expected", FILES).expect("materialize");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).expect("chmod");

        let rebuilt = materialize_embedded_sources_at(&parent, "expected", FILES)
            .expect("rebuild private cache");
        assert_eq!(
            fs::metadata(rebuilt).unwrap().permissions().mode() & 0o077,
            0
        );
    }
}
