#[cfg(test)]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;

use super::{RepoFileReadError, normalize_repo_relative_input};

#[derive(Debug)]
pub(crate) struct RepoDirectory;

#[derive(Debug)]
pub(crate) struct RepoFile;

#[derive(Debug)]
pub(crate) struct RepoDirectoryEntry {
    pub(crate) name: OsString,
    pub(crate) kind: Result<RepoDirectoryEntryKind, RepoFileReadError>,
}

#[derive(Debug)]
pub(crate) enum RepoDirectoryEntryKind {
    Directory(RepoDirectory),
    File(RepoFile),
    Other,
}

impl RepoDirectory {
    pub(crate) fn open(root: &Path, relative_path: &Path) -> Result<Self, RepoFileReadError> {
        let relative_path =
            normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
        match std::fs::symlink_metadata(root) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(RepoFileReadError::SecureOpenUnavailable),
            Err(error) => return Err(map_fallback_metadata_error(error, true)),
        }
        let mut candidate = root.to_path_buf();
        for component in relative_path.components() {
            candidate.push(component.as_os_str());
            match std::fs::symlink_metadata(&candidate) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(RepoFileReadError::NotFound);
                }
                Err(error) => return Err(map_fallback_metadata_error(error, false)),
                Ok(metadata)
                    if metadata.file_type().is_symlink()
                        || (!metadata.is_dir() && candidate != root.join(&relative_path)) =>
                {
                    return Err(RepoFileReadError::SecureOpenUnavailable);
                }
                Ok(_) => {}
            }
        }
        Err(RepoFileReadError::SecureOpenUnavailable)
    }

    #[cfg(test)]
    pub(crate) fn open_file(&self, _name: &OsStr) -> Result<RepoFile, RepoFileReadError> {
        Err(RepoFileReadError::SecureOpenUnavailable)
    }

    pub(crate) fn visit_entries(
        &self,
        _visitor: impl FnMut(RepoDirectoryEntry) -> bool,
    ) -> Result<(), RepoFileReadError> {
        Err(RepoFileReadError::SecureOpenUnavailable)
    }
}

fn map_fallback_metadata_error(error: std::io::Error, root: bool) -> RepoFileReadError {
    if error.kind() == std::io::ErrorKind::OutOfMemory {
        return RepoFileReadError::ResourceExhausted;
    }
    #[cfg(unix)]
    if matches!(
        error.raw_os_error(),
        Some(libc::ENOMEM) | Some(libc::EMFILE) | Some(libc::ENFILE)
    ) {
        return RepoFileReadError::ResourceExhausted;
    }
    if root {
        RepoFileReadError::RootUnavailable
    } else {
        RepoFileReadError::SecureOpenUnavailable
    }
}

impl RepoFile {
    pub(crate) fn read_with_limit(self, _max_bytes: u64) -> Result<Vec<u8>, RepoFileReadError> {
        Err(RepoFileReadError::SecureOpenUnavailable)
    }

    pub(crate) fn read_to_string_with_limit(
        self,
        _max_bytes: u64,
    ) -> Result<String, RepoFileReadError> {
        Err(RepoFileReadError::SecureOpenUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_optional_directory_is_distinct_from_present_unsupported_directory() {
        let temp = tempfile::tempdir().expect("tempdir");

        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::NotFound)
        ));

        std::fs::create_dir_all(temp.path().join(".polint/models"))
            .expect("create model directory");
        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::SecureOpenUnavailable)
        ));
    }

    #[test]
    fn metadata_allocation_failure_is_resource_exhaustion() {
        assert!(matches!(
            map_fallback_metadata_error(
                std::io::Error::from(std::io::ErrorKind::OutOfMemory),
                false,
            ),
            RepoFileReadError::ResourceExhausted
        ));
        #[cfg(unix)]
        assert!(matches!(
            map_fallback_metadata_error(std::io::Error::from_raw_os_error(libc::ENOMEM), true),
            RepoFileReadError::ResourceExhausted
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_intermediate_link_is_not_reported_as_an_absent_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        symlink(outside.path(), temp.path().join(".polint")).expect("create intermediate link");

        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::SecureOpenUnavailable)
        ));
    }
}
