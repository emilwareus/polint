use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem::{align_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path};
use std::sync::Arc;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_INFORMATION, FILE_OPEN, FILE_OPEN_FOR_BACKUP_INTENT, FILE_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, FileDirectoryInformation, NtCreateFile, NtQueryDirectoryFile,
};
use windows_sys::Win32::Foundation::{
    ERROR_CANT_ACCESS_FILE, ERROR_DIRECTORY, ERROR_FILE_NOT_FOUND, ERROR_INVALID_NAME,
    ERROR_NO_SYSTEM_RESOURCES, ERROR_NOT_ENOUGH_MEMORY, ERROR_NOT_ENOUGH_QUOTA, ERROR_OUTOFMEMORY,
    ERROR_PATH_NOT_FOUND, ERROR_REPARSE, ERROR_REPARSE_OBJECT, ERROR_REPARSE_POINT_ENCOUNTERED,
    ERROR_REPARSE_TAG_INVALID, ERROR_REPARSE_TAG_MISMATCH, ERROR_STOPPED_ON_SYMLINK,
    ERROR_TOO_MANY_OPEN_FILES, ERROR_WORKING_SET_QUOTA, HANDLE, INVALID_HANDLE_VALUE,
    RtlNtStatusToDosError, STATUS_BUFFER_OVERFLOW, STATUS_NO_MORE_FILES, STATUS_NO_SUCH_FILE,
    STATUS_PENDING, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FileAttributeTagInfo, FileIdInfo, GetFileInformationByHandleEx, GetFinalPathNameByHandleW,
    OPEN_EXISTING, ReOpenFile, SYNCHRONIZE, VOLUME_NAME_GUID,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use super::{RepoFileReadError, normalize_repo_relative_input, read_open_file_with_limit};

const DIRECTORY_QUERY_BUFFER_BYTES: usize = 64 * 1_024;
const FINAL_PATH_BUFFER_UNITS: usize = 32_768;
const REPO_COMPONENT_OBJECT_ATTRIBUTES: u32 = 0;

#[derive(Debug)]
pub(crate) struct RepoDirectory {
    file: Arc<fs::File>,
    identity: FileIdentity,
    root: Arc<RootAnchor>,
}

#[derive(Debug)]
pub(crate) struct RepoFile {
    file: fs::File,
    identity: FileIdentity,
    root: Arc<RootAnchor>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileIdentity {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

#[derive(Debug)]
struct RootAnchor {
    file: Arc<fs::File>,
    identity: FileIdentity,
    final_path: NormalizedFinalPath,
}

#[derive(Debug)]
struct HandleState {
    identity: FileIdentity,
    final_path: NormalizedFinalPath,
    attributes: u32,
    is_directory: bool,
    is_file: bool,
    file_len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NormalizedFinalPath {
    volume: Vec<Vec<u16>>,
    components: Vec<Vec<u16>>,
}

impl RepoDirectory {
    pub(crate) fn open(root: &Path, relative_path: &Path) -> Result<Self, RepoFileReadError> {
        if !root.is_absolute() {
            return Err(RepoFileReadError::RootUnavailable);
        }
        let relative_path =
            normalize_repo_relative_input(relative_path).ok_or(RepoFileReadError::AbsolutePath)?;
        let root_file = Arc::new(open_root(root)?);
        let state = inspect_handle(&root_file)?;
        if state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(RepoFileReadError::EscapesRepo);
        }
        if !state.is_directory {
            return Err(RepoFileReadError::RootUnavailable);
        }
        let root = Arc::new(RootAnchor {
            file: Arc::clone(&root_file),
            identity: state.identity.clone(),
            final_path: state.final_path,
        });
        let mut directory = Self {
            file: root_file,
            identity: state.identity,
            root,
        };
        for component in relative_path.components() {
            let Component::Normal(name) = component else {
                return Err(RepoFileReadError::AbsolutePath);
            };
            directory = match directory.open_entry(name)? {
                RepoDirectoryEntryKind::Directory(directory) => directory,
                RepoDirectoryEntryKind::File(_) | RepoDirectoryEntryKind::Other => {
                    return Err(RepoFileReadError::NotDirectory);
                }
            };
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
        &self,
        mut visitor: impl FnMut(RepoDirectoryEntry) -> bool,
    ) -> Result<(), RepoFileReadError> {
        self.verify()?;
        let enumeration_file = reopen_directory(raw_handle(&self.file))?;
        let enumeration_state = inspect_handle(&enumeration_file)?;
        if enumeration_state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !enumeration_state.is_directory
            || enumeration_state.identity != self.identity
            || !enumeration_state
                .final_path
                .is_within(&self.root.final_path)
        {
            return Err(RepoFileReadError::EscapesRepo);
        }
        let mut storage = vec![0_u64; DIRECTORY_QUERY_BUFFER_BYTES / size_of::<u64>()];
        let mut restart_scan = true;
        loop {
            storage.fill(0);
            let mut io_status = IO_STATUS_BLOCK::default();
            let status = query_next_directory_entry(
                raw_handle(&enumeration_file),
                &mut io_status,
                &mut storage,
                restart_scan,
            );
            let first_query = restart_scan;
            restart_scan = false;
            if status == STATUS_NO_MORE_FILES || (first_query && status == STATUS_NO_SUCH_FILE) {
                self.verify()?;
                return Ok(());
            }
            if status == STATUS_BUFFER_OVERFLOW || status == STATUS_PENDING {
                return Err(RepoFileReadError::Read);
            }
            if status < 0 {
                return Err(map_ntstatus(status));
            }
            self.verify()?;
            let name = parse_directory_entry_name(&storage, io_status.Information)?;
            if is_dot_entry(&name) {
                continue;
            }
            let name = OsString::from_wide(&name);
            let kind = self.open_entry(&name);
            if !visitor(RepoDirectoryEntry { name, kind }) {
                self.verify()?;
                return Ok(());
            }
        }
    }

    fn open_entry(&self, name: &OsStr) -> Result<RepoDirectoryEntryKind, RepoFileReadError> {
        self.verify()?;
        let file = open_relative(raw_handle(&self.file), name)?;
        let state = inspect_handle(&file)?;
        if state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(RepoFileReadError::EscapesRepo);
        }
        if !state.final_path.is_within(&self.root.final_path) {
            return Err(RepoFileReadError::EscapesRepo);
        }
        self.verify()?;
        if state.is_directory {
            Ok(RepoDirectoryEntryKind::Directory(Self {
                file: Arc::new(file),
                identity: state.identity,
                root: Arc::clone(&self.root),
            }))
        } else if state.is_file {
            Ok(RepoDirectoryEntryKind::File(RepoFile {
                file,
                identity: state.identity,
                root: Arc::clone(&self.root),
            }))
        } else {
            Ok(RepoDirectoryEntryKind::Other)
        }
    }

    fn verify(&self) -> Result<(), RepoFileReadError> {
        self.root.verify()?;
        let state = inspect_handle(&self.file)?;
        if state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !state.is_directory
            || state.identity != self.identity
            || !state.final_path.is_within(&self.root.final_path)
        {
            return Err(RepoFileReadError::EscapesRepo);
        }
        Ok(())
    }
}

impl RepoFile {
    pub(crate) fn read_with_limit(mut self, max_bytes: u64) -> Result<Vec<u8>, RepoFileReadError> {
        let before = self.verify()?;
        let bytes = read_open_file_with_limit(&mut self.file, before.file_len, max_bytes)?;
        self.verify()?;
        Ok(bytes)
    }

    pub(crate) fn read_to_string_with_limit(
        self,
        max_bytes: u64,
    ) -> Result<String, RepoFileReadError> {
        String::from_utf8(self.read_with_limit(max_bytes)?)
            .map_err(|_| RepoFileReadError::InvalidUtf8)
    }

    fn verify(&self) -> Result<HandleState, RepoFileReadError> {
        self.root.verify()?;
        let state = inspect_handle(&self.file)?;
        if state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !state.is_file
            || state.identity != self.identity
            || !state.final_path.is_within(&self.root.final_path)
        {
            return Err(RepoFileReadError::EscapesRepo);
        }
        Ok(state)
    }
}

impl RootAnchor {
    fn verify(&self) -> Result<(), RepoFileReadError> {
        let state = inspect_handle(&self.file)?;
        if state.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || !state.is_directory
            || state.identity != self.identity
            || state.final_path != self.final_path
        {
            return Err(RepoFileReadError::EscapesRepo);
        }
        Ok(())
    }
}

impl NormalizedFinalPath {
    fn parse(path: &[u16]) -> Result<Self, RepoFileReadError> {
        let (is_unc, remainder) = if strip_ascii_prefix(path, r"\\?\UNC\").is_some() {
            (true, strip_ascii_prefix(path, r"\\?\UNC\").unwrap_or(path))
        } else if let Some(remainder) = strip_ascii_prefix(path, r"\\?\") {
            (false, remainder)
        } else if let Some(remainder) = strip_ascii_prefix(path, r"\\") {
            (true, remainder)
        } else {
            (false, path)
        };
        let mut parts = remainder
            .split(|unit| *unit == u16::from(b'\\') || *unit == u16::from(b'/'))
            .filter(|part| !part.is_empty())
            .map(<[u16]>::to_vec)
            .collect::<Vec<_>>();
        let volume_count = if is_unc { 2 } else { 1 };
        if parts.len() < volume_count {
            return Err(RepoFileReadError::Read);
        }
        let mut volume = parts.drain(..volume_count).collect::<Vec<_>>();
        for component in &mut volume {
            for unit in component {
                if let Ok(byte) = u8::try_from(*unit)
                    && byte.is_ascii_uppercase()
                {
                    *unit = u16::from(byte.to_ascii_lowercase());
                }
            }
        }
        if is_unc {
            volume.insert(0, "unc".encode_utf16().collect());
        }
        if parts.iter().any(|part| {
            part.as_slice() == [u16::from(b'.')]
                || part.as_slice() == [u16::from(b'.'), u16::from(b'.')]
        }) {
            return Err(RepoFileReadError::Read);
        }
        Ok(Self {
            volume,
            components: parts,
        })
    }

    fn is_within(&self, root: &Self) -> bool {
        self.volume == root.volume
            && self.components.len() >= root.components.len()
            && self.components[..root.components.len()] == root.components
    }
}

fn strip_ascii_prefix<'a>(path: &'a [u16], prefix: &str) -> Option<&'a [u16]> {
    let prefix = prefix.encode_utf16().collect::<Vec<_>>();
    (path.len() >= prefix.len()
        && path[..prefix.len()]
            .iter()
            .zip(&prefix)
            .all(|(left, right)| {
                u8::try_from(*left)
                    .ok()
                    .zip(u8::try_from(*right).ok())
                    .is_some_and(|(left, right)| left.eq_ignore_ascii_case(&right))
            }))
    .then(|| &path[prefix.len()..])
}

fn is_dot_entry(name: &[u16]) -> bool {
    name == [u16::from(b'.')] || name == [u16::from(b'.'), u16::from(b'.')]
}

fn raw_handle(file: &fs::File) -> HANDLE {
    file.as_raw_handle() as HANDLE
}

#[allow(unsafe_code)]
fn reopen_directory(directory: HANDLE) -> Result<fs::File, RepoFileReadError> {
    // SAFETY: `directory` is a live verified handle. A successful independent
    // enumeration handle is transferred exactly once into `File` below.
    let handle = unsafe {
        ReOpenFile(
            directory,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(map_read_error(std::io::Error::last_os_error()));
    }
    // SAFETY: `ReOpenFile` returned a new owned handle on success.
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

#[allow(unsafe_code)]
fn open_root(root: &Path) -> Result<fs::File, RepoFileReadError> {
    let mut path = root.as_os_str().encode_wide().collect::<Vec<_>>();
    if path.contains(&0) {
        return Err(RepoFileReadError::RootUnavailable);
    }
    path.push(0);
    // SAFETY: the path is NUL-terminated for the duration of the call. A
    // successful handle is transferred exactly once into `File` below.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(map_root_open_error(std::io::Error::last_os_error()));
    }
    // SAFETY: `CreateFileW` returned a new owned handle on success.
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

#[allow(unsafe_code)]
fn open_relative(parent: HANDLE, name: &OsStr) -> Result<fs::File, RepoFileReadError> {
    let mut name = name.encode_wide().collect::<Vec<_>>();
    if name.is_empty()
        || name == [u16::from(b'.')]
        || name == [u16::from(b'.'), u16::from(b'.')]
        || name.iter().any(|unit| {
            *unit == 0
                || *unit == u16::from(b'\\')
                || *unit == u16::from(b'/')
                || *unit == u16::from(b':')
        })
    {
        return Err(RepoFileReadError::EscapesRepo);
    }
    let byte_len = name
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u16::try_from(length).ok())
        .ok_or(RepoFileReadError::Read)?;
    let object_name = UNICODE_STRING {
        Length: byte_len,
        MaximumLength: byte_len,
        Buffer: name.as_mut_ptr(),
    };
    let object_attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent,
        ObjectName: &object_name,
        Attributes: REPO_COMPONENT_OBJECT_ATTRIBUTES,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut handle: HANDLE = std::ptr::null_mut();
    let mut io_status = IO_STATUS_BLOCK::default();
    // SAFETY: all input structures and the single-component name remain live
    // for the call. The parent is a verified directory handle, and a successful
    // output handle is transferred exactly once into `File` below.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &object_attributes,
            &mut io_status,
            std::ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status == STATUS_PENDING {
        if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
            // SAFETY: `NtCreateFile` initialized a handle that must be released.
            drop(unsafe { fs::File::from_raw_handle(handle) });
        }
        return Err(RepoFileReadError::Read);
    }
    if status < 0 {
        return Err(map_ntstatus(status));
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(RepoFileReadError::Read);
    }
    // SAFETY: `NtCreateFile` returned a new owned handle on success.
    Ok(unsafe { fs::File::from_raw_handle(handle) })
}

#[allow(unsafe_code)]
fn inspect_handle(file: &fs::File) -> Result<HandleState, RepoFileReadError> {
    let handle = raw_handle(file);
    let mut tag = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: `tag` is a correctly sized writable output buffer and the handle
    // remains live for the call.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            (&raw mut tag).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    } == 0
    {
        return Err(map_metadata_error(std::io::Error::last_os_error()));
    }
    if tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(RepoFileReadError::EscapesRepo);
    }
    let mut identity = FILE_ID_INFO::default();
    // SAFETY: `identity` is a correctly sized writable output buffer and the
    // handle remains live for the call.
    if unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdInfo,
            (&raw mut identity).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(map_metadata_error(std::io::Error::last_os_error()));
    }
    let metadata = file.metadata().map_err(map_metadata_error)?;
    Ok(HandleState {
        identity: FileIdentity {
            volume_serial_number: identity.VolumeSerialNumber,
            file_id: identity.FileId.Identifier,
        },
        final_path: final_path(handle)?,
        attributes: tag.FileAttributes,
        is_directory: metadata.is_dir() && tag.FileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
        is_file: metadata.is_file() && tag.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0,
        file_len: metadata.len(),
    })
}

#[allow(unsafe_code)]
fn final_path(handle: HANDLE) -> Result<NormalizedFinalPath, RepoFileReadError> {
    for flags in [0, VOLUME_NAME_GUID] {
        let mut buffer = vec![0_u16; FINAL_PATH_BUFFER_UNITS];
        // SAFETY: `buffer` is writable for its declared length and `handle`
        // remains live for the call.
        let length = unsafe {
            GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, flags)
        };
        if length == 0 {
            let error = map_read_error(std::io::Error::last_os_error());
            if error.is_resource_exhausted() {
                return Err(error);
            }
            continue;
        }
        let length = usize::try_from(length).map_err(|_| RepoFileReadError::Read)?;
        if length < buffer.len() {
            return NormalizedFinalPath::parse(&buffer[..length]);
        }
    }
    Err(RepoFileReadError::Read)
}

#[allow(unsafe_code)]
fn query_next_directory_entry(
    directory: HANDLE,
    io_status: &mut IO_STATUS_BLOCK,
    storage: &mut [u64],
    restart_scan: bool,
) -> i32 {
    // SAFETY: the directory is a verified synchronous directory handle;
    // `storage` and `io_status` are writable and remain live for the call.
    unsafe {
        NtQueryDirectoryFile(
            directory,
            std::ptr::null_mut(),
            None,
            std::ptr::null(),
            io_status,
            storage.as_mut_ptr().cast(),
            std::mem::size_of_val(storage) as u32,
            FileDirectoryInformation,
            true,
            std::ptr::null(),
            restart_scan,
        )
    }
}

#[allow(unsafe_code)]
fn parse_directory_entry_name(
    storage: &[u64],
    returned_bytes: usize,
) -> Result<Vec<u16>, RepoFileReadError> {
    let storage_bytes = std::mem::size_of_val(storage);
    let name_offset = std::mem::offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
    if returned_bytes < name_offset || returned_bytes > storage_bytes {
        return Err(RepoFileReadError::Read);
    }
    let record_address = storage.as_ptr() as usize;
    if record_address % align_of::<FILE_DIRECTORY_INFORMATION>() != 0 {
        return Err(RepoFileReadError::Read);
    }
    // SAFETY: alignment and the minimum returned record size were validated.
    let record = unsafe { &*storage.as_ptr().cast::<FILE_DIRECTORY_INFORMATION>() };
    if record.NextEntryOffset != 0 {
        let next = record.NextEntryOffset as usize;
        if next % align_of::<FILE_DIRECTORY_INFORMATION>() != 0
            || next < name_offset
            || next > returned_bytes
        {
            return Err(RepoFileReadError::Read);
        }
        return Err(RepoFileReadError::Read);
    }
    let name_bytes = record.FileNameLength as usize;
    if name_bytes == 0 || name_bytes % size_of::<u16>() != 0 {
        return Err(RepoFileReadError::Read);
    }
    let name_end = name_offset
        .checked_add(name_bytes)
        .filter(|end| *end <= returned_bytes)
        .ok_or(RepoFileReadError::Read)?;
    let name_address = record_address
        .checked_add(name_offset)
        .ok_or(RepoFileReadError::Read)?;
    if name_address % align_of::<u16>() != 0 || name_end > storage_bytes {
        return Err(RepoFileReadError::Read);
    }
    // SAFETY: the byte length, alignment, and returned-buffer bounds were
    // validated above; the record owns these UTF-16 code units.
    Ok(unsafe {
        std::slice::from_raw_parts(name_address as *const u16, name_bytes / size_of::<u16>())
    }
    .to_vec())
}

fn map_root_open_error(error: std::io::Error) -> RepoFileReadError {
    match error.raw_os_error().map(|code| code as u32) {
        Some(code) if is_resource_exhaustion(code) => RepoFileReadError::ResourceExhausted,
        Some(ERROR_CANT_ACCESS_FILE)
        | Some(ERROR_REPARSE)
        | Some(ERROR_REPARSE_OBJECT)
        | Some(ERROR_REPARSE_POINT_ENCOUNTERED)
        | Some(ERROR_REPARSE_TAG_INVALID)
        | Some(ERROR_REPARSE_TAG_MISMATCH)
        | Some(ERROR_STOPPED_ON_SYMLINK) => RepoFileReadError::EscapesRepo,
        Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PATH_NOT_FOUND) => {
            RepoFileReadError::RootUnavailable
        }
        _ => RepoFileReadError::RootUnavailable,
    }
}

#[allow(unsafe_code)]
fn map_ntstatus(status: i32) -> RepoFileReadError {
    // SAFETY: converting an NTSTATUS to its stable Win32 error code has no
    // pointer or ownership preconditions.
    match unsafe { RtlNtStatusToDosError(status) } {
        code if is_resource_exhaustion(code) => RepoFileReadError::ResourceExhausted,
        ERROR_CANT_ACCESS_FILE
        | ERROR_REPARSE
        | ERROR_REPARSE_OBJECT
        | ERROR_REPARSE_POINT_ENCOUNTERED
        | ERROR_REPARSE_TAG_INVALID
        | ERROR_REPARSE_TAG_MISMATCH
        | ERROR_STOPPED_ON_SYMLINK => RepoFileReadError::EscapesRepo,
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => RepoFileReadError::NotFound,
        ERROR_DIRECTORY => RepoFileReadError::NotDirectory,
        ERROR_INVALID_NAME => RepoFileReadError::EscapesRepo,
        _ => RepoFileReadError::Read,
    }
}

fn map_read_error(error: std::io::Error) -> RepoFileReadError {
    match error.raw_os_error().map(|code| code as u32) {
        Some(code) if is_resource_exhaustion(code) => RepoFileReadError::ResourceExhausted,
        _ => RepoFileReadError::Read,
    }
}

fn map_metadata_error(error: std::io::Error) -> RepoFileReadError {
    match error.raw_os_error().map(|code| code as u32) {
        Some(code) if is_resource_exhaustion(code) => RepoFileReadError::ResourceExhausted,
        _ => RepoFileReadError::Metadata,
    }
}

fn is_resource_exhaustion(code: u32) -> bool {
    matches!(
        code,
        ERROR_TOO_MANY_OPEN_FILES
            | ERROR_NOT_ENOUGH_MEMORY
            | ERROR_OUTOFMEMORY
            | ERROR_NO_SYSTEM_RESOURCES
            | ERROR_WORKING_SET_QUOTA
            | ERROR_NOT_ENOUGH_QUOTA
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::process::Command;

    use super::*;

    #[test]
    fn handle_exhaustion_has_a_distinct_error() {
        assert!(matches!(
            map_read_error(std::io::Error::from_raw_os_error(
                ERROR_TOO_MANY_OPEN_FILES as i32,
            )),
            RepoFileReadError::ResourceExhausted
        ));
    }

    fn enable_case_sensitive_directory(path: &Path) -> bool {
        let output = Command::new("fsutil")
            .args(["file", "setCaseSensitiveInfo"])
            .arg(path)
            .arg("enable")
            .output()
            .expect("run fsutil");
        if output.status.success() {
            return true;
        }
        assert_ne!(
            std::env::var_os("POLINT_REQUIRE_WINDOWS_CASE_SENSITIVE_TEST").as_deref(),
            Some(OsStr::new("1")),
            "CI requires Windows case-sensitive-directory coverage: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!(
            "skipping privileged Windows case-sensitive-directory integration check: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        false
    }

    #[test]
    fn repo_components_request_exact_nt_name_matching() {
        assert_eq!(REPO_COMPONENT_OBJECT_ATTRIBUTES, 0);
    }

    #[test]
    fn enumerated_names_are_reopened_exactly_in_case_sensitive_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        if !enable_case_sensitive_directory(temp.path()) {
            return;
        }
        fs::write(temp.path().join("Rules.toml"), "name = 'upper'\n").expect("write upper");
        fs::write(temp.path().join("rules.toml"), "name = 'lower'\n").expect("write lower");
        let directory = RepoDirectory::open(temp.path(), Path::new(".")).expect("open directory");
        let mut contents = BTreeMap::new();

        directory
            .visit_entries(|entry| {
                if let Ok(RepoDirectoryEntryKind::File(file)) = entry.kind {
                    contents.insert(
                        entry.name,
                        file.read_to_string_with_limit(1_024).expect("read file"),
                    );
                }
                true
            })
            .expect("enumerate directory");

        assert_eq!(
            contents
                .get(&OsString::from("Rules.toml"))
                .map(String::as_str),
            Some("name = 'upper'\n")
        );
        assert_eq!(
            contents
                .get(&OsString::from("rules.toml"))
                .map(String::as_str),
            Some("name = 'lower'\n")
        );
    }

    #[test]
    fn fixed_repo_components_are_opened_with_exact_case() {
        let temp = tempfile::tempdir().expect("tempdir");
        if !enable_case_sensitive_directory(temp.path()) {
            return;
        }
        fs::create_dir_all(temp.path().join(".POLINT/models")).expect("wrong-case model directory");

        assert!(matches!(
            RepoDirectory::open(temp.path(), Path::new(".polint/models")),
            Err(RepoFileReadError::NotFound)
        ));

        fs::create_dir_all(temp.path().join(".polint/models")).expect("exact model directory");
        RepoDirectory::open(temp.path(), Path::new(".polint/models"))
            .expect("exact component path remains unambiguous when both casings exist");
    }
}
