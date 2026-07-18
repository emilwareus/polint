use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs;
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path};
use std::sync::Arc;

use super::{RepoFileReadError, normalize_repo_relative_input, read_open_file_with_limit};

#[derive(Debug)]
pub(crate) struct RepoDirectory {
    file: fs::File,
    boundary: Arc<RepoBoundary>,
}

#[derive(Debug)]
pub(crate) struct RepoFile {
    file: fs::File,
    boundary: Arc<RepoBoundary>,
}

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

#[derive(Debug)]
struct RepoBoundary {
    device: u64,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    mount_id: u64,
}

impl RepoDirectory {
    pub(crate) fn open(root: &Path, relative_path: &Path) -> Result<Self, RepoFileReadError> {
        let relative_path =
            normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
        let root = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(root)
            .map_err(map_root_open_error)?;
        let metadata = root.metadata().map_err(map_metadata_error)?;
        if !metadata.is_dir() {
            return Err(RepoFileReadError::RootUnavailable);
        }
        let boundary = Arc::new(RepoBoundary::new(&root, &metadata)?);

        let mut directory = Self {
            file: root,
            boundary,
        };
        for component in relative_path.components() {
            let Component::Normal(name) = component else {
                return Err(RepoFileReadError::AbsolutePath);
            };
            directory = directory.open_requested_directory(name)?;
        }
        Ok(directory)
    }

    #[cfg(test)]
    pub(crate) fn open_file(&self, name: &OsStr) -> Result<RepoFile, RepoFileReadError> {
        match self.open_entry(name)? {
            RepoDirectoryEntryKind::File(file) => Ok(file),
            RepoDirectoryEntryKind::Directory(_) | RepoDirectoryEntryKind::Other => {
                Err(RepoFileReadError::NotFile)
            }
        }
    }

    pub(crate) fn visit_entries(
        &mut self,
        mut visitor: impl FnMut(RepoDirectoryEntry) -> bool,
    ) -> Result<(), RepoFileReadError> {
        let descriptor = open_directory_for_enumeration(self.file.as_raw_fd())?;
        let directory = open_directory_stream(descriptor)?;
        loop {
            let Some(name) = directory.next_name()? else {
                return Ok(());
            };
            if name.as_bytes() == b"." || name.as_bytes() == b".." {
                continue;
            }
            let kind = self.open_entry(&name);
            if !visitor(RepoDirectoryEntry { name, kind }) {
                return Ok(());
            }
        }
    }

    fn open_requested_directory(&self, name: &OsStr) -> Result<Self, RepoFileReadError> {
        let directory = match self.open_entry(name)? {
            RepoDirectoryEntryKind::Directory(directory) => directory,
            RepoDirectoryEntryKind::File(_) | RepoDirectoryEntryKind::Other => {
                return Err(RepoFileReadError::NotDirectory);
            }
        };
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        verify_exact_component_name(&directory.file, name)?;
        Ok(directory)
    }

    fn open_entry(&self, name: &OsStr) -> Result<RepoDirectoryEntryKind, RepoFileReadError> {
        if name.as_bytes().is_empty()
            || name.as_bytes() == b"."
            || name.as_bytes() == b".."
            || name.as_bytes().contains(&b'/')
        {
            return Err(RepoFileReadError::EscapesRepo);
        }
        let name = CString::new(name.as_bytes()).map_err(|_| RepoFileReadError::Metadata)?;
        let prechecked = precheck_entry(self.file.as_raw_fd(), &name)?;
        if prechecked.device != self.boundary.device {
            return Err(RepoFileReadError::EscapesRepo);
        }
        let flags = match prechecked.kind {
            PrecheckedType::Directory => {
                libc::O_RDONLY
                    | libc::O_CLOEXEC
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_DIRECTORY
            }
            PrecheckedType::File => {
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK
            }
            PrecheckedType::Other => return Ok(RepoDirectoryEntryKind::Other),
        };
        let descriptor = openat_no_follow(self.file.as_raw_fd(), &name, flags)
            .map_err(map_anchored_open_error)?;
        let file = fs::File::from(descriptor);
        let metadata = file.metadata().map_err(map_metadata_error)?;
        if metadata.dev() != prechecked.device || metadata.ino() != prechecked.inode {
            return Err(RepoFileReadError::EscapesRepo);
        }
        self.boundary.verify(&file, &metadata)?;
        if prechecked.kind == PrecheckedType::Directory && metadata.is_dir() {
            Ok(RepoDirectoryEntryKind::Directory(Self {
                file,
                boundary: Arc::clone(&self.boundary),
            }))
        } else if prechecked.kind == PrecheckedType::File && metadata.is_file() {
            Ok(RepoDirectoryEntryKind::File(RepoFile {
                file,
                boundary: Arc::clone(&self.boundary),
            }))
        } else {
            Err(RepoFileReadError::EscapesRepo)
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unsafe_code)]
fn verify_exact_component_name(
    file: &fs::File,
    expected_name: &OsStr,
) -> Result<(), RepoFileReadError> {
    let mut path = [0_u8; libc::PATH_MAX as usize];
    // SAFETY: `file` owns a live descriptor and `path` is a writable
    // `PATH_MAX`-sized buffer, as required by Darwin's `F_GETPATH` command.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, path.as_mut_ptr()) } < 0 {
        return Err(map_metadata_error(std::io::Error::last_os_error()));
    }
    let path_length = path
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(RepoFileReadError::Metadata)?;
    let actual_name = Path::new(OsStr::from_bytes(&path[..path_length]))
        .file_name()
        .ok_or(RepoFileReadError::Metadata)?;
    if actual_name == expected_name {
        Ok(())
    } else {
        Err(RepoFileReadError::NotFound)
    }
}

impl RepoFile {
    pub(crate) fn read_with_limit(mut self, max_bytes: u64) -> Result<Vec<u8>, RepoFileReadError> {
        let metadata = self.file.metadata().map_err(map_metadata_error)?;
        if !metadata.is_file() {
            return Err(RepoFileReadError::NotFile);
        }
        self.boundary.verify(&self.file, &metadata)?;
        let bytes = read_open_file_with_limit(&mut self.file, metadata.len(), max_bytes)?;
        let metadata = self.file.metadata().map_err(map_metadata_error)?;
        self.boundary.verify(&self.file, &metadata)?;
        Ok(bytes)
    }

    pub(crate) fn read_to_string_with_limit(
        self,
        max_bytes: u64,
    ) -> Result<String, RepoFileReadError> {
        String::from_utf8(self.read_with_limit(max_bytes)?)
            .map_err(|_| RepoFileReadError::InvalidUtf8)
    }
}

impl RepoBoundary {
    fn new(file: &fs::File, metadata: &fs::Metadata) -> Result<Self, RepoFileReadError> {
        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        let _ = file;
        Ok(Self {
            device: metadata.dev(),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            mount_id: file_mount_id(file)?,
        })
    }

    fn verify(&self, file: &fs::File, metadata: &fs::Metadata) -> Result<(), RepoFileReadError> {
        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        let _ = file;
        if metadata.dev() != self.device {
            return Err(RepoFileReadError::EscapesRepo);
        }
        #[cfg(any(target_os = "android", target_os = "linux"))]
        self.verify_mount_id(file_mount_id(file)?)?;
        Ok(())
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn verify_mount_id(&self, mount_id: u64) -> Result<(), RepoFileReadError> {
        if mount_id == self.mount_id {
            Ok(())
        } else {
            Err(RepoFileReadError::EscapesRepo)
        }
    }
}

fn map_root_open_error(error: std::io::Error) -> RepoFileReadError {
    if error.kind() == std::io::ErrorKind::OutOfMemory {
        return RepoFileReadError::ResourceExhausted;
    }
    match error.raw_os_error() {
        Some(libc::EMFILE) | Some(libc::ENFILE) | Some(libc::ENOMEM) => {
            RepoFileReadError::ResourceExhausted
        }
        Some(code) if nofollow_symlink_error(code) => RepoFileReadError::EscapesRepo,
        Some(libc::ENOTDIR) => RepoFileReadError::EscapesRepo,
        _ => RepoFileReadError::RootUnavailable,
    }
}

fn map_anchored_open_error(error: std::io::Error) -> RepoFileReadError {
    if error.kind() == std::io::ErrorKind::OutOfMemory {
        return RepoFileReadError::ResourceExhausted;
    }
    match error.raw_os_error() {
        Some(libc::EMFILE) | Some(libc::ENFILE) | Some(libc::ENOMEM) => {
            RepoFileReadError::ResourceExhausted
        }
        Some(libc::ENOENT) => RepoFileReadError::NotFound,
        Some(code) if nofollow_symlink_error(code) => RepoFileReadError::EscapesRepo,
        Some(libc::ENOTDIR) => RepoFileReadError::NotDirectory,
        _ => RepoFileReadError::Read,
    }
}

fn nofollow_symlink_error(code: libc::c_int) -> bool {
    if matches!(code, libc::ELOOP | libc::EMLINK) {
        return true;
    }
    #[cfg(target_os = "netbsd")]
    if code == libc::EFTYPE {
        return true;
    }
    false
}

fn map_metadata_error(error: std::io::Error) -> RepoFileReadError {
    if error.kind() == std::io::ErrorKind::OutOfMemory {
        return RepoFileReadError::ResourceExhausted;
    }
    match error.raw_os_error() {
        Some(libc::EMFILE) | Some(libc::ENFILE) | Some(libc::ENOMEM) => {
            RepoFileReadError::ResourceExhausted
        }
        _ => RepoFileReadError::Metadata,
    }
}

#[allow(unsafe_code)]
fn openat_no_follow(directory: RawFd, name: &CStr, flags: libc::c_int) -> std::io::Result<OwnedFd> {
    // SAFETY: `directory` is a live directory descriptor, `name` is NUL-terminated,
    // and ownership of a successful descriptor is transferred exactly once.
    let descriptor = unsafe { libc::openat(directory, name.as_ptr(), flags) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `openat` returned a new owned descriptor on success.
    Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrecheckedType {
    Directory,
    File,
    Other,
}

#[derive(Clone, Copy, Debug)]
struct PrecheckedEntry {
    kind: PrecheckedType,
    device: u64,
    inode: u64,
}

#[allow(
    unsafe_code,
    clippy::unnecessary_cast,
    reason = "libc dev_t and ino_t widths vary across supported Unix targets"
)]
fn precheck_entry(directory: RawFd, name: &CStr) -> Result<PrecheckedEntry, RepoFileReadError> {
    // SAFETY: a zeroed `stat` is a valid output buffer, and both the directory
    // descriptor and NUL-terminated name remain live for the call.
    let mut status = unsafe { std::mem::zeroed::<libc::stat>() };
    // SAFETY: the pointers are valid and `AT_SYMLINK_NOFOLLOW` requests metadata
    // for the entry itself rather than traversing a symbolic link.
    if unsafe {
        libc::fstatat(
            directory,
            name.as_ptr(),
            &mut status,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(map_anchored_open_error(std::io::Error::last_os_error()));
    }
    let kind = match status.st_mode & libc::S_IFMT {
        libc::S_IFDIR => PrecheckedType::Directory,
        libc::S_IFREG => PrecheckedType::File,
        libc::S_IFLNK => return Err(RepoFileReadError::EscapesRepo),
        _ => PrecheckedType::Other,
    };
    Ok(PrecheckedEntry {
        kind,
        device: status.st_dev as u64,
        inode: status.st_ino as u64,
    })
}

fn open_directory_for_enumeration(directory: RawFd) -> Result<OwnedFd, RepoFileReadError> {
    let current = c".";
    let flags =
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_DIRECTORY;
    openat_no_follow(directory, current, flags).map_err(map_anchored_open_error)
}

struct DirectoryStream(*mut libc::DIR);

impl DirectoryStream {
    #[allow(unsafe_code)]
    fn next_name(&self) -> Result<Option<OsString>, RepoFileReadError> {
        clear_errno();
        // SAFETY: the stream remains live for `self`; `readdir` owns its internal
        // buffer and the returned name is copied before the next call.
        let entry = unsafe { libc::readdir(self.0) };
        if entry.is_null() {
            return if current_errno() == 0 {
                Ok(None)
            } else {
                Err(map_anchored_open_error(std::io::Error::from_raw_os_error(
                    current_errno(),
                )))
            };
        }
        // SAFETY: POSIX guarantees that a successful `readdir` result contains a
        // NUL-terminated `d_name` valid until the next operation on this stream.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        Ok(Some(OsString::from_vec(name.to_bytes().to_vec())))
    }
}

impl Drop for DirectoryStream {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: this stream exclusively owns the pointer returned by
        // `fdopendir`, and it is closed exactly once here.
        unsafe {
            libc::closedir(self.0);
        }
    }
}

#[allow(unsafe_code)]
fn open_directory_stream(descriptor: OwnedFd) -> Result<DirectoryStream, RepoFileReadError> {
    let raw_descriptor = descriptor.into_raw_fd();
    // SAFETY: ownership of `raw_descriptor` transfers to the returned DIR on
    // success. On failure, `fdopendir` leaves it owned by the caller.
    let stream = unsafe { libc::fdopendir(raw_descriptor) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        // SAFETY: `fdopendir` failed, so the descriptor was not consumed.
        unsafe {
            libc::close(raw_descriptor);
        }
        Err(map_anchored_open_error(error))
    } else {
        Ok(DirectoryStream(stream))
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[allow(unsafe_code)]
fn file_mount_id(file: &fs::File) -> Result<u64, RepoFileReadError> {
    #[cfg(any(target_os = "android", all(target_os = "linux", target_env = "gnu")))]
    {
        // SAFETY: a zeroed `statx` is a valid output buffer, the descriptor
        // remains live, and `AT_EMPTY_PATH` makes the empty path refer to it.
        let mut status = unsafe { std::mem::zeroed::<libc::statx>() };
        // SAFETY: using the syscall entry point avoids depending on a libc
        // wrapper version. All pointers remain valid for the call.
        let result = unsafe {
            libc::syscall(
                libc::SYS_statx,
                file.as_raw_fd(),
                c"".as_ptr(),
                libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
                libc::STATX_MNT_ID,
                &mut status,
            ) as libc::c_int
        };
        if result == 0 && status.stx_mask & libc::STATX_MNT_ID != 0 {
            return Ok(status.stx_mnt_id);
        }
    }
    proc_fdinfo_mount_id(file)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn proc_fdinfo_mount_id(file: &fs::File) -> Result<u64, RepoFileReadError> {
    const MAX_FDINFO_BYTES: u64 = 4 * 1_024;

    let path = format!("/proc/self/fdinfo/{}", file.as_raw_fd());
    let fdinfo = fs::File::open(path).map_err(map_metadata_error)?;
    let mut bytes = Vec::new();
    fdinfo
        .take(MAX_FDINFO_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(map_metadata_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FDINFO_BYTES {
        return Err(RepoFileReadError::Metadata);
    }
    let inode = file.metadata().map_err(map_metadata_error)?.ino();
    parse_fdinfo_mount_id(&bytes, inode)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn parse_fdinfo_mount_id(bytes: &[u8], expected_inode: u64) -> Result<u64, RepoFileReadError> {
    let value = std::str::from_utf8(bytes).map_err(|_| RepoFileReadError::Metadata)?;
    let mut mount_id = None;
    let mut inode = None;
    for line in value.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let target = match name {
            "mnt_id" => &mut mount_id,
            "ino" => &mut inode,
            _ => continue,
        };
        let value = value
            .trim()
            .parse::<u64>()
            .map_err(|_| RepoFileReadError::Metadata)?;
        if target.replace(value).is_some() {
            return Err(RepoFileReadError::Metadata);
        }
    }
    if inode != Some(expected_inode) {
        return Err(RepoFileReadError::Metadata);
    }
    mount_id.ok_or(RepoFileReadError::Metadata)
}

#[cfg(any(target_os = "dragonfly", target_os = "linux"))]
#[allow(unsafe_code)]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: libc returns the current thread's errno storage.
    unsafe { libc::__errno_location() }
}

#[cfg(any(target_os = "freebsd", target_os = "ios", target_os = "macos"))]
#[allow(unsafe_code)]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: libc returns the current thread's errno storage.
    unsafe { libc::__error() }
}

#[cfg(any(target_os = "android", target_os = "netbsd", target_os = "openbsd"))]
#[allow(unsafe_code)]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: libc returns the current thread's errno storage.
    unsafe { libc::__errno() }
}

#[allow(unsafe_code)]
fn clear_errno() {
    // SAFETY: `errno_location` points to writable thread-local errno storage.
    unsafe {
        *errno_location() = 0;
    }
}

#[allow(unsafe_code)]
fn current_errno() -> libc::c_int {
    // SAFETY: `errno_location` points to readable thread-local errno storage.
    unsafe { *errno_location() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn requested_repo_metadata_directory_requires_exact_case() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join(".POLINT/models")).expect("wrong-case model directory");

        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::NotFound)
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn requested_model_directory_requires_exact_case() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join(".polint/Models")).expect("wrong-case model directory");

        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::NotFound)
        ));
    }

    #[test]
    fn directory_stream_names_remain_owned_across_reads() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("first.toml"), "first = true\n").expect("write first");
        fs::write(temp.path().join("second.toml"), "second = true\n").expect("write second");
        let directory = RepoDirectory::open(temp.path(), Path::new(".")).expect("open directory");
        let descriptor =
            open_directory_for_enumeration(directory.file.as_raw_fd()).expect("open enumeration");
        let stream = open_directory_stream(descriptor).expect("directory stream");
        let mut retained = Vec::new();
        while let Some(name) = stream.next_name().expect("read name") {
            if name.as_bytes() != b"." && name.as_bytes() != b".." {
                retained.push(name);
                if retained.len() == 2 {
                    break;
                }
            }
        }
        retained.sort();

        assert_eq!(
            retained,
            [OsString::from("first.toml"), OsString::from("second.toml")]
        );
    }

    #[test]
    fn process_resource_exhaustion_has_a_distinct_error() {
        for map in [
            map_root_open_error,
            map_anchored_open_error,
            map_metadata_error,
        ] {
            assert!(matches!(
                map(std::io::Error::from(std::io::ErrorKind::OutOfMemory)),
                RepoFileReadError::ResourceExhausted
            ));
        }
        for code in [libc::EMFILE, libc::ENFILE, libc::ENOMEM] {
            assert!(matches!(
                map_root_open_error(std::io::Error::from_raw_os_error(code)),
                RepoFileReadError::ResourceExhausted
            ));
            assert!(matches!(
                map_anchored_open_error(std::io::Error::from_raw_os_error(code)),
                RepoFileReadError::ResourceExhausted
            ));
            assert!(matches!(
                map_metadata_error(std::io::Error::from_raw_os_error(code)),
                RepoFileReadError::ResourceExhausted
            ));
        }
    }

    #[test]
    fn platform_nofollow_symlink_errors_escape_the_repository() {
        for code in [libc::ELOOP, libc::EMLINK] {
            assert!(matches!(
                map_root_open_error(std::io::Error::from_raw_os_error(code)),
                RepoFileReadError::EscapesRepo
            ));
            assert!(matches!(
                map_anchored_open_error(std::io::Error::from_raw_os_error(code)),
                RepoFileReadError::EscapesRepo
            ));
        }
        #[cfg(target_os = "netbsd")]
        for map in [map_root_open_error, map_anchored_open_error] {
            assert!(matches!(
                map(std::io::Error::from_raw_os_error(libc::EFTYPE)),
                RepoFileReadError::EscapesRepo
            ));
        }
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn same_device_different_mount_is_outside_the_repo_boundary() {
        let boundary = RepoBoundary {
            device: 7,
            mount_id: 11,
        };

        assert!(matches!(
            boundary.verify_mount_id(12),
            Err(RepoFileReadError::EscapesRepo)
        ));
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn fdinfo_fallback_requires_unique_mount_and_matching_inode() {
        let valid = b"pos:\t0\nflags:\t0100000\nmnt_id:\t42\nino:\t9001\n";

        assert_eq!(
            parse_fdinfo_mount_id(valid, 9001).expect("valid fdinfo"),
            42
        );
        assert!(parse_fdinfo_mount_id(valid, 9002).is_err());
        assert!(parse_fdinfo_mount_id(b"ino:\t9001\n", 9001).is_err());
        assert!(parse_fdinfo_mount_id(b"mnt_id:\t42\nmnt_id:\t43\nino:\t9001\n", 9001).is_err());
    }
}
