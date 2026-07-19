//! Windows-only filesystem and process-containment primitives.
//!
//! Raw Win32 calls are kept behind safe wrappers so the semantic frontend can
//! fail closed without spreading handle or pointer invariants through the
//! process and filesystem orchestration code.

use std::cell::Cell;
use std::ffi::OsString;
use std::fs::{File, Metadata};
use std::io::{self, Read, Write};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ALREADY_EXISTS, ERROR_BAD_LENGTH,
    ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_FILES, ERROR_NOT_FOUND, ERROR_SUCCESS, GENERIC_READ,
    GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, ACL_SIZE_INFORMATION, AclSizeInformation,
    AddAccessAllowedAceEx, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
    GetAclInformation, GetLengthSid, GetSecurityDescriptorControl, GetTokenInformation,
    InitializeAcl, InitializeSecurityDescriptor, IsValidSid, OBJECT_INHERIT_ACE,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SetSecurityDescriptorControl,
    SetSecurityDescriptorDacl, SetSecurityDescriptorOwner, TOKEN_OWNER, TOKEN_QUERY, TOKEN_USER,
    TokenOwner, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO, FILE_WRITE_ATTRIBUTES,
    FileAttributeTagInfo, FileBasicInfo, FileIdInfo, FileStandardInfo, GetDriveTypeW,
    GetFileInformationByHandleEx, GetFinalPathNameByHandleW, GetVolumeInformationW,
    GetVolumePathNameW, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL, SetFileInformationByHandle,
    VOLUME_NAME_GUID, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThread,
    ResumeThread, THREAD_SUSPEND_RESUME,
};
use windows_sys::Win32::UI::Shell::UrlCreateFromPathW;

const DRIVE_FIXED: u32 = 3;
const MAX_WINDOWS_PATH_UNITS: usize = 32_768;
const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const MAX_TOOLHELP_SNAPSHOT_RETRIES: usize = 16;
const MAX_TOOLHELP_THREAD_ENTRIES: usize = 262_144;
const FILE_IO_CANCELLATION_GRACE: Duration = Duration::from_millis(100);
const LOCAL_TREE_CERTIFICATION_TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_TREE_MAX_ENTRIES: usize = 1_000_000;
const LOCAL_TREE_MAX_DEPTH: usize = 256;
const LOCAL_TREE_MAX_FRONTIER: usize = 65_536;
const LOCAL_TREE_MAX_FRONTIER_PATH_UNITS: usize = 32 * 1_048_576;
const LOCAL_TREE_MAX_SCOPED_PATHS: usize = LOCAL_TREE_MAX_ENTRIES + 1;
const LOCAL_TREE_MAX_SCOPED_PATH_UNITS: usize = 64 * 1_048_576;
const FINAL_PATH_BUFFER_UNITS: usize = 32_768;
const MAX_WINDOWS_COMMAND_CWD_UNITS: usize = 260;

thread_local! {
    /// Set only on a worker whose real thread handle is owned by the caller.
    /// Nested secure-file operations can therefore run inline while retaining
    /// the outer pass's `CancelSynchronousIo` deadline enforcement.
    static CANCELLABLE_FILE_IO_PASS_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
thread_local! {
    static FILE_IO_WORKER_SPAWNS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WindowsFileIdentity {
    pub(super) volume_serial_number: u64,
    pub(super) file_id: [u8; 16],
    pub(super) size: u64,
    pub(super) creation_time: i64,
    pub(super) last_write_time: i64,
    pub(super) change_time: i64,
    pub(super) attributes: u32,
    pub(super) directory: bool,
}

impl WindowsFileIdentity {
    fn names_same_object(self, other: Self) -> bool {
        self.volume_serial_number == other.volume_serial_number
            && self.file_id == other.file_id
            && self.directory == other.directory
            && self.attributes & FILE_ATTRIBUTE_REPARSE_POINT
                == other.attributes & FILE_ATTRIBUTE_REPARSE_POINT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WindowsEffectiveAccess(u8);

impl WindowsEffectiveAccess {
    const READ_EXECUTE: Self = Self(0b11);

    pub(super) const fn projection(self) -> u32 {
        self.0 as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WindowsPrivateAccess {
    Mutable,
    Sealed,
}

impl WindowsPrivateAccess {
    pub(super) const fn projection(self) -> u32 {
        match self {
            Self::Mutable => 0,
            Self::Sealed => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WindowsScopedFileState {
    pub(super) identity: WindowsFileIdentity,
    pub(super) effective_access: WindowsEffectiveAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedFinalPath {
    volume: Vec<Vec<u16>>,
    components: Vec<Vec<u16>>,
}

#[derive(Debug)]
pub(super) struct CertifiedLocalTree {
    root: PathBuf,
    root_handle: File,
    root_identity: WindowsFileIdentity,
    root_final_path: NormalizedFinalPath,
    owner_thread: thread::ThreadId,
    deadline: Instant,
}

#[derive(Debug)]
pub(super) struct StableFileContents {
    pub(super) bytes: Vec<u8>,
    pub(super) sha256: [u8; 32],
    pub(super) identity: WindowsFileIdentity,
}

#[derive(Debug)]
pub(super) struct SecureFile {
    file: File,
    identity: WindowsFileIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FinalReparsePolicy {
    Reject,
    AllowDirectChild,
}

impl CertifiedLocalTree {
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn identity_no_follow(&self, path: &Path) -> io::Result<WindowsFileIdentity> {
        let (handle, identity) = self.open_scoped_entry(
            path,
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            FinalReparsePolicy::Reject,
        )?;
        drop(handle);
        Ok(identity)
    }

    /// Captures a direct poisoned destination without following its final
    /// reparse point. This is intentionally narrower than normal identity
    /// capture so quarantine code can name an unsafe file, directory, or link
    /// without making it a trusted input.
    pub(super) fn direct_child_identity_allow_reparse(
        &self,
        path: &Path,
    ) -> io::Result<WindowsFileIdentity> {
        let (handle, identity) = self.open_scoped_entry(
            path,
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            FinalReparsePolicy::AllowDirectChild,
        )?;
        drop(handle);
        Ok(identity)
    }

    pub(super) fn open_regular_no_follow(&self, path: &Path) -> io::Result<SecureFile> {
        let (handle, identity) = self.open_scoped_entry(
            path,
            GENERIC_READ | READ_CONTROL,
            FILE_SHARE_READ,
            Some(false),
            FinalReparsePolicy::Reject,
        )?;
        Ok(SecureFile {
            file: file_from_handle(handle),
            identity,
        })
    }

    pub(super) fn read_execute_state(
        &self,
        path: &Path,
        directory: bool,
    ) -> io::Result<WindowsScopedFileState> {
        let (handle, identity) = self.open_scoped_entry(
            path,
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE | READ_CONTROL | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(directory),
            FinalReparsePolicy::Reject,
        )?;
        drop(handle);
        Ok(WindowsScopedFileState {
            identity,
            effective_access: WindowsEffectiveAccess::READ_EXECUTE,
        })
    }

    pub(super) fn private_state(
        &self,
        path: &Path,
        directory: bool,
    ) -> io::Result<(WindowsFileIdentity, WindowsPrivateAccess)> {
        let (handle, identity) = self.open_scoped_entry(
            path,
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(directory),
            FinalReparsePolicy::Reject,
        )?;
        let access = private_handle_access(handle.as_raw_handle().cast(), directory)?;
        drop(handle);
        Ok((identity, access))
    }

    fn open_scoped_entry(
        &self,
        path: &Path,
        access: u32,
        share: u32,
        expected_directory: Option<bool>,
        final_reparse: FinalReparsePolicy,
    ) -> io::Result<(OwnedHandle, WindowsFileIdentity)> {
        const OPERATION: &str = "Windows root-scoped identity capture";

        self.require_active(OPERATION)?;
        self.verify_root_binding()?;
        let candidate = scoped_absolute_path(path)?;
        if candidate != self.root && !candidate.starts_with(&self.root) {
            return Err(invalid_data(
                path,
                "the entry is outside its certified local-tree root",
            ));
        }
        if final_reparse == FinalReparsePolicy::AllowDirectChild
            && candidate.parent() != Some(self.root.as_path())
        {
            return Err(invalid_data(
                path,
                "unsafe final-entry identity is limited to a direct child of its certified root",
            ));
        }
        let flags = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT;
        let handle = open_existing_path(&candidate, access, share, flags)?;
        let identity = handle_identity(handle.as_raw_handle().cast())?;
        if final_reparse == FinalReparsePolicy::Reject {
            reject_reparse_identity(&candidate, identity)?;
        }
        require_same_scoped_volume(&candidate, self.root_identity, identity)?;
        if let Some(directory) = expected_directory
            && identity.directory != directory
        {
            return Err(invalid_data(
                &candidate,
                "the entry type changed during root-scoped capture",
            ));
        }
        if final_reparse == FinalReparsePolicy::Reject {
            let final_path = final_path(handle.as_raw_handle().cast())?;
            let expected_final_path = self.expected_final_path(&candidate)?;
            if !final_path.is_within(&self.root_final_path) || final_path != expected_final_path {
                return Err(invalid_data(
                    &candidate,
                    "the entry escaped its certified local-tree root through a reparse boundary",
                ));
            }
        }
        self.verify_root_binding()?;
        Ok((handle, identity))
    }

    fn expected_final_path(&self, candidate: &Path) -> io::Result<NormalizedFinalPath> {
        let relative = candidate.strip_prefix(&self.root).map_err(|_| {
            invalid_data(
                candidate,
                "the entry is outside its certified local-tree root",
            )
        })?;
        let mut expected = self.root_final_path.clone();
        for component in relative.components() {
            let std::path::Component::Normal(component) = component else {
                return Err(invalid_data(
                    candidate,
                    "the entry contains an invalid root-scoped path component",
                ));
            };
            expected
                .components
                .push(component.encode_wide().collect::<Vec<_>>());
        }
        Ok(expected)
    }

    fn require_active(&self, operation: &str) -> io::Result<()> {
        require_file_io_before_deadline(self.deadline, operation)?;
        if !cancellable_file_io_pass_is_active() || thread::current().id() != self.owner_thread {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a certified Windows tree scope may only be used inside its owning cancellable filesystem pass",
            ));
        }
        Ok(())
    }

    fn verify_root_binding(&self) -> io::Result<()> {
        let identity = handle_identity(file_handle(&self.root_handle))?;
        reject_reparse_identity(&self.root, identity)?;
        if !identity.names_same_object(self.root_identity)
            || final_path(file_handle(&self.root_handle))? != self.root_final_path
        {
            return Err(invalid_data(
                &self.root,
                "the certified local-tree root changed identity",
            ));
        }
        Ok(())
    }
}

fn require_same_scoped_volume(
    path: &Path,
    root: WindowsFileIdentity,
    candidate: WindowsFileIdentity,
) -> io::Result<()> {
    if candidate.volume_serial_number != root.volume_serial_number {
        return Err(invalid_data(
            path,
            "the entry escaped onto a different Windows volume",
        ));
    }
    Ok(())
}

impl SecureFile {
    pub(super) fn open_regular_no_follow(path: &Path) -> io::Result<Self> {
        require_local_fixed_volume(path)?;
        Self::open_regular_no_follow_in_certified_tree(path)
    }

    /// Opens a descendant after its root was recursively certified on the
    /// active cancellable worker. Callers must not use this for arbitrary
    /// paths: it deliberately avoids repeating volume/ancestor certification.
    pub(super) fn open_regular_no_follow_in_certified_tree(path: &Path) -> io::Result<Self> {
        let file = file_from_handle(open_existing_path(
            path,
            GENERIC_READ | READ_CONTROL,
            FILE_SHARE_READ,
            FILE_FLAG_OPEN_REPARSE_POINT,
        )?);
        let identity = handle_identity(file_handle(&file))?;
        reject_reparse_identity(path, identity)?;
        if identity.directory {
            return Err(invalid_data(
                path,
                "expected a regular file, found a directory",
            ));
        }
        Ok(Self { file, identity })
    }

    pub(super) fn identity(&self) -> WindowsFileIdentity {
        self.identity
    }

    pub(super) fn verify_unchanged(&self) -> io::Result<()> {
        let current = handle_identity(file_handle(&self.file))?;
        if current != self.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the pinned Windows file changed after it was opened",
            ));
        }
        Ok(())
    }

    pub(super) fn read_bounded_until(
        self,
        limit: usize,
        deadline: Instant,
    ) -> io::Result<StableFileContents> {
        if cancellable_file_io_pass_is_active() {
            return self.read_bounded_inner(limit, deadline);
        }
        run_cancellable_file_io(deadline, "Windows file reading", move || {
            self.read_bounded_inner(limit, deadline)
        })
    }

    fn read_bounded_inner(
        mut self,
        limit: usize,
        deadline: Instant,
    ) -> io::Result<StableFileContents> {
        let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
        let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
        let mut remaining = read_limit;
        let mut buffer = [0_u8; 64 * 1024];
        while remaining != 0 {
            require_file_io_before_deadline(deadline, "Windows file reading")?;
            let chunk = usize::try_from(remaining)
                .unwrap_or(usize::MAX)
                .min(buffer.len());
            let count = self.file.read(&mut buffer[..chunk])?;
            if count == 0 {
                break;
            }
            remaining = remaining.saturating_sub(u64::try_from(count).unwrap_or(u64::MAX));
            if count > limit.saturating_sub(bytes.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("the pinned Windows file exceeds the {limit}-byte read limit"),
                ));
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        self.verify_unchanged()?;
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        Ok(StableFileContents {
            bytes,
            sha256,
            identity: self.identity,
        })
    }

    pub(super) fn hash_into_until(
        self,
        hasher: &mut Sha256,
        limit: u64,
        deadline: Instant,
    ) -> io::Result<u64> {
        let initial_hasher = hasher.clone();
        let (updated_hasher, bytes_read) = if cancellable_file_io_pass_is_active() {
            self.hash_into_inner(initial_hasher, limit, deadline)?
        } else {
            run_cancellable_file_io(deadline, "Windows file hashing", move || {
                self.hash_into_inner(initial_hasher, limit, deadline)
            })?
        };
        *hasher = updated_hasher;
        Ok(bytes_read)
    }

    fn hash_into_inner(
        mut self,
        mut hasher: Sha256,
        limit: u64,
        deadline: Instant,
    ) -> io::Result<(Sha256, u64)> {
        if self.identity.size > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the pinned Windows file exceeds the {limit}-byte hashing limit"),
            ));
        }
        let mut bytes_read = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            require_file_io_before_deadline(deadline, "Windows file hashing")?;
            let count = self.file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes_read = bytes_read.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
            if bytes_read > self.identity.size || bytes_read > limit {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the pinned Windows file changed while it was hashed",
                ));
            }
            hasher.update(&buffer[..count]);
        }
        self.verify_unchanged()?;
        if bytes_read != self.identity.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the pinned Windows file changed while it was hashed",
            ));
        }
        Ok((hasher, bytes_read))
    }
}

#[derive(Clone, Debug)]
pub(super) struct PinnedDirectoryGuard {
    handle: Arc<File>,
    identity: WindowsFileIdentity,
    path: Arc<PathBuf>,
}

impl PinnedDirectoryGuard {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        require_local_fixed_volume(path)?;
        let file = file_from_handle(open_existing_path(
            path,
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        )?);
        let identity = handle_identity(file_handle(&file))?;
        reject_reparse_identity(path, identity)?;
        if !identity.directory {
            return Err(invalid_data(path, "expected a directory, found a file"));
        }
        verify_private_handle(file_handle(&file), true, false)?;
        Ok(Self {
            handle: Arc::new(file),
            identity,
            path: Arc::new(path.to_path_buf()),
        })
    }

    pub(super) fn identity(&self) -> WindowsFileIdentity {
        self.identity
    }

    pub(super) fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub(super) fn verify_path_binding(&self) -> io::Result<()> {
        verify_private_handle(file_handle(&self.handle), true, false)?;
        if !handle_identity(file_handle(&self.handle))?.names_same_object(self.identity) {
            return Err(invalid_data(
                &self.path,
                "the pinned directory handle changed identity",
            ));
        }
        let candidate = Self::open(&self.path)?;
        if !candidate.identity.names_same_object(self.identity) {
            return Err(invalid_data(
                &self.path,
                "the directory path no longer names the pinned directory",
            ));
        }
        Ok(())
    }
}

pub(super) fn path_is_reparse_point(path: &Path) -> io::Result<bool> {
    std::fs::symlink_metadata(path).map(|metadata| is_reparse_point(&metadata))
}

pub(super) fn is_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(super) fn require_local_fixed_volume(path: &Path) -> io::Result<()> {
    let absolute = preflight_local_drive(path)?;
    reject_reparse_ancestors(&absolute)?;
    require_endpoint_local_volume(&absolute)
}

/// Rejects remote/unsupported volumes before callers inspect a candidate's
/// nearest existing ancestor. The full containing-path gate must still run
/// once that ancestor is known so reparse points are rejected.
pub(super) fn require_local_creation_volume(path: &Path) -> io::Result<PathBuf> {
    preflight_local_drive(path)
}

/// Converts an already-certified local path to the spelling Win32 exposes to
/// child processes without changing its component semantics.
pub(super) fn go_command_path(path: &Path) -> io::Result<PathBuf> {
    use std::path::{Component, Prefix};

    validate_windows_path_units(path)?;
    if path.to_str().is_none() {
        return Err(invalid_data(
            path,
            "Go command paths must contain valid Unicode",
        ));
    }
    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) => prefix,
        _ => {
            return Err(invalid_data(
                path,
                "Go command paths must use an absolute local drive path",
            ));
        }
    };
    let strip_verbatim_prefix = match prefix.kind() {
        Prefix::Disk(_) => false,
        Prefix::VerbatimDisk(_) => true,
        Prefix::UNC(_, _)
        | Prefix::VerbatimUNC(_, _)
        | Prefix::DeviceNS(_)
        | Prefix::Verbatim(_) => {
            return Err(invalid_data(
                path,
                "Go command paths must use a local drive prefix",
            ));
        }
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(invalid_data(path, "Go command paths must be absolute"));
    }
    for component in components {
        let Component::Normal(name) = component else {
            return Err(invalid_data(
                path,
                "Go command paths must not contain relative components",
            ));
        };
        validate_go_command_component(path, name)?;
    }

    let mut units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if strip_verbatim_prefix {
        let prefix = [
            u16::from(b'\\'),
            u16::from(b'\\'),
            u16::from(b'?'),
            u16::from(b'\\'),
        ];
        units = units
            .strip_prefix(&prefix)
            .ok_or_else(|| invalid_data(path, "invalid verbatim local drive prefix"))?
            .to_vec();
    }
    for unit in &mut units {
        if *unit == u16::from(b'/') {
            *unit = u16::from(b'\\');
        }
    }
    Ok(PathBuf::from(OsString::from_wide(&units)))
}

pub(super) fn go_command_working_directory(path: &Path) -> io::Result<PathBuf> {
    let command_path = go_command_path(path)?;
    let units = command_path.as_os_str().encode_wide().collect::<Vec<_>>();
    let terminator_units = if units.last() == Some(&u16::from(b'\\')) {
        1
    } else {
        2
    };
    if units.len().saturating_add(terminator_units) > MAX_WINDOWS_COMMAND_CWD_UNITS {
        return Err(invalid_data(
            path,
            "Go command working directory exceeds the Win32 process limit",
        ));
    }
    Ok(command_path)
}

fn validate_go_command_component(path: &Path, component: &std::ffi::OsStr) -> io::Result<()> {
    let units = component.encode_wide().collect::<Vec<_>>();
    if units.is_empty()
        || units
            .last()
            .is_some_and(|unit| [u16::from(b'.'), u16::from(b' ')].contains(unit))
        || units.iter().copied().any(is_invalid_go_command_unit)
        || is_reserved_dos_component(&units)
    {
        return Err(invalid_data(
            path,
            "a path component changes meaning at the Win32 command boundary",
        ));
    }
    Ok(())
}

fn is_invalid_go_command_unit(unit: u16) -> bool {
    unit < 0x20
        || matches!(
            unit,
            0x22 | 0x2A | 0x2F | 0x3A | 0x3C | 0x3E | 0x3F | 0x5C | 0x7C
        )
}

fn is_reserved_dos_component(component: &[u16]) -> bool {
    let basename_end = component
        .iter()
        .position(|unit| *unit == u16::from(b'.'))
        .unwrap_or(component.len());
    let basename = &component[..basename_end];
    let basename_end = basename
        .iter()
        .rposition(|unit| *unit != u16::from(b' '))
        .map_or(0, |index| index.saturating_add(1));
    let basename = &basename[..basename_end];
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|reserved| utf16_eq_ascii_ignore_case(basename, reserved.as_bytes()))
    {
        return true;
    }
    if basename.len() != 4 {
        return false;
    }
    let device = &basename[..3];
    let numbered_device =
        utf16_eq_ascii_ignore_case(device, b"COM") || utf16_eq_ascii_ignore_case(device, b"LPT");
    numbered_device
        && matches!(
            basename[3],
            value if (u16::from(b'1')..=u16::from(b'9')).contains(&value)
                || matches!(value, 0x00B9 | 0x00B2 | 0x00B3)
        )
}

fn utf16_eq_ascii_ignore_case(units: &[u16], ascii: &[u8]) -> bool {
    units.len() == ascii.len()
        && units.iter().zip(ascii).all(|(unit, byte)| {
            let upper = if (u16::from(b'a')..=u16::from(b'z')).contains(unit) {
                *unit - u16::from(b'a') + u16::from(b'A')
            } else {
                *unit
            };
            upper == u16::from(byte.to_ascii_uppercase())
        })
}

fn preflight_local_drive(path: &Path) -> io::Result<PathBuf> {
    validate_windows_path_units(path)?;
    let absolute = std::path::absolute(path)?;
    validate_windows_path_units(&absolute)?;
    reject_alternate_data_streams(&absolute)?;
    let drive_root = local_drive_root(&absolute)?;
    require_supported_volume(path, &drive_root)?;
    Ok(absolute)
}

fn local_drive_root(path: &Path) -> io::Result<Vec<u16>> {
    use std::path::{Component, Prefix};

    let prefix = match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => {
            return Err(invalid_data(
                path,
                "raw Windows filesystem operations require a local drive path",
            ));
        }
    };
    let drive = match prefix {
        Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
        Prefix::UNC(_, _)
        | Prefix::VerbatimUNC(_, _)
        | Prefix::DeviceNS(_)
        | Prefix::Verbatim(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "`{}` is not an accepted local Windows drive path",
                    path.display()
                ),
            ));
        }
    };
    Ok(vec![u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0])
}

fn require_endpoint_local_volume(path: &Path) -> io::Result<()> {
    let path_wide = wide_path(path)?;
    let mut volume_path = vec![0_u16; MAX_WINDOWS_PATH_UNITS];
    get_volume_path(&path_wide, &mut volume_path)?;
    truncate_at_nul(&mut volume_path);
    if volume_path.is_empty() {
        return Err(invalid_data(
            path,
            "Windows did not return a containing volume",
        ));
    }
    volume_path.push(0);
    require_supported_volume(path, &volume_path)
}

fn require_supported_volume(path: &Path, volume_path_nul: &[u16]) -> io::Result<()> {
    if get_drive_type(volume_path_nul) != DRIVE_FIXED {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "`{}` is not on a fixed local Windows volume",
                path.display()
            ),
        ));
    }

    let mut filesystem = [0_u16; 64];
    get_volume_information(volume_path_nul, &mut filesystem)?;
    let filesystem = String::from_utf16_lossy(
        &filesystem[..filesystem
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(filesystem.len())],
    );
    if !matches!(filesystem.to_ascii_uppercase().as_str(), "NTFS" | "REFS") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "`{}` is on unsupported Windows filesystem `{filesystem}`; only NTFS and ReFS are accepted",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct LocalTreeCertificationLimits {
    entries: usize,
    depth: usize,
    frontier: usize,
    frontier_path_units: usize,
}

#[derive(Debug)]
struct ScopedPathIndex {
    paths: Vec<PathBuf>,
}

impl ScopedPathIndex {
    fn new(paths: &[PathBuf], deadline: Instant, operation: &str) -> io::Result<Self> {
        validate_scoped_path_batch(paths, deadline, operation)?;
        let mut indexed = Vec::new();
        indexed.try_reserve_exact(paths.len()).map_err(|error| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("could not allocate the bounded Windows scoped-path index: {error}"),
            )
        })?;
        let mut indexed_units = 0_usize;
        for path in paths {
            require_file_io_before_deadline(deadline, operation)?;
            let path = scoped_absolute_path(path)?;
            indexed_units = indexed_units
                .checked_add(path.as_os_str().encode_wide().count())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Windows local-tree indexed-path accounting overflowed",
                    )
                })?;
            if indexed_units > LOCAL_TREE_MAX_SCOPED_PATH_UNITS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Windows local-tree index exceeds its {LOCAL_TREE_MAX_SCOPED_PATH_UNITS}-unit path limit"
                    ),
                ));
            }
            indexed.push(path);
        }
        require_file_io_before_deadline(deadline, operation)?;
        indexed.sort_unstable_by(|left, right| compare_path_components(left, right));
        require_file_io_before_deadline(deadline, operation)?;
        indexed.dedup_by(|left, right| {
            compare_path_components(left, right) == std::cmp::Ordering::Equal
        });
        Ok(Self { paths: indexed })
    }

    fn contains_exact(&self, path: &Path) -> bool {
        self.paths
            .binary_search_by(|candidate| compare_path_components(candidate, path))
            .is_ok()
    }

    fn contains_descendant_of(&self, path: &Path) -> bool {
        let index = self
            .paths
            .partition_point(|candidate| compare_path_components(candidate, path).is_lt());
        self.paths
            .get(index)
            .is_some_and(|candidate| candidate.starts_with(path))
    }

    fn contains_ancestor_of(&self, path: &Path, root: &Path) -> bool {
        path.ancestors()
            .take_while(|ancestor| ancestor.starts_with(root))
            .any(|ancestor| self.contains_exact(ancestor))
    }
}

fn compare_path_components(left: &Path, right: &Path) -> std::cmp::Ordering {
    left.components()
        .map(|component| component.as_os_str())
        .cmp(right.components().map(|component| component.as_os_str()))
}

fn validate_scoped_path_batch(
    paths: &[PathBuf],
    deadline: Instant,
    operation: &str,
) -> io::Result<()> {
    require_file_io_before_deadline(deadline, operation)?;
    if paths.len() > LOCAL_TREE_MAX_SCOPED_PATHS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Windows local-tree scope exceeds its {LOCAL_TREE_MAX_SCOPED_PATHS}-path limit"
            ),
        ));
    }
    let mut units = 0_usize;
    for path in paths {
        require_file_io_before_deadline(deadline, operation)?;
        validate_windows_path_units(path)?;
        units = units
            .checked_add(path.as_os_str().encode_wide().count())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows local-tree scoped-path accounting overflowed",
                )
            })?;
        if units > LOCAL_TREE_MAX_SCOPED_PATH_UNITS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Windows local-tree scope exceeds its {LOCAL_TREE_MAX_SCOPED_PATH_UNITS}-unit path limit"
                ),
            ));
        }
    }
    require_file_io_before_deadline(deadline, operation)
}

impl LocalTreeCertificationLimits {
    const DEFAULT: Self = Self {
        entries: LOCAL_TREE_MAX_ENTRIES,
        depth: LOCAL_TREE_MAX_DEPTH,
        frontier: LOCAL_TREE_MAX_FRONTIER,
        frontier_path_units: LOCAL_TREE_MAX_FRONTIER_PATH_UNITS,
    };
}

/// Certifies that an existing local tree contains no descendant reparse point.
///
/// This remains a point-in-time check, but all potentially blocking metadata
/// operations execute on the cancellable Windows file-I/O worker. The walk is
/// also bounded independently of the caller's deadline so a broad or deeply
/// nested tree fails closed instead of consuming unbounded memory.
pub(super) fn require_local_tree(path: &Path) -> io::Result<()> {
    let deadline = Instant::now()
        .checked_add(LOCAL_TREE_CERTIFICATION_TIMEOUT)
        .unwrap_or_else(Instant::now);
    require_local_tree_until(path, deadline)
}

pub(super) fn require_local_tree_until(path: &Path, deadline: Instant) -> io::Result<()> {
    require_local_tree_with_exclusions_until(path, &[], deadline)
}

pub(super) fn require_local_tree_with_exclusions_until(
    path: &Path,
    exclusions: &[PathBuf],
    deadline: Instant,
) -> io::Result<()> {
    require_local_tree_with_exclusions_and_limits_until(
        path,
        exclusions,
        &[],
        false,
        deadline,
        LocalTreeCertificationLimits::DEFAULT,
    )
}

pub(super) fn require_local_tree_with_scope_until(
    path: &Path,
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    deadline: Instant,
) -> io::Result<()> {
    require_local_tree_with_exclusions_and_limits_until(
        path,
        exclusions,
        inclusions,
        true,
        deadline,
        LocalTreeCertificationLimits::DEFAULT,
    )
}

/// Returns a root-scoped capture handle for use by the active cancellable
/// filesystem pass. Unlike the Go-package scope variant, this scans every
/// descendant directory other than explicit exclusion trees.
pub(super) fn certified_local_tree_until(
    path: &Path,
    exclusions: &[PathBuf],
    deadline: Instant,
) -> io::Result<CertifiedLocalTree> {
    if !cancellable_file_io_pass_is_active() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "certified Windows tree scopes require an active cancellable filesystem pass",
        ));
    }
    require_local_tree_inner(
        path,
        exclusions,
        &[],
        false,
        deadline,
        LocalTreeCertificationLimits::DEFAULT,
    )
}

/// Pins only one local directory. Descendants are not trusted; the resulting
/// scope exposes the narrow direct-child identity operation used to quarantine
/// a poisoned published entry without traversing it.
pub(super) fn certified_local_directory_until(
    path: &Path,
    deadline: Instant,
) -> io::Result<CertifiedLocalTree> {
    if !cancellable_file_io_pass_is_active() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "certified Windows directory scopes require an active cancellable filesystem pass",
        ));
    }
    certified_local_directory_inner(path, deadline)
}

/// Returns a root-scoped capture handle while applying Go's ignored-directory
/// traversal rules. `inclusions` force explicitly selected package ancestors
/// through otherwise ignored `testdata`, dot, or underscore directories.
pub(super) fn certified_local_tree_with_scope_until(
    path: &Path,
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    deadline: Instant,
) -> io::Result<CertifiedLocalTree> {
    if !cancellable_file_io_pass_is_active() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "certified Windows tree scopes require an active cancellable filesystem pass",
        ));
    }
    require_local_tree_inner(
        path,
        exclusions,
        inclusions,
        true,
        deadline,
        LocalTreeCertificationLimits::DEFAULT,
    )
}

fn require_local_tree_with_limits_until(
    path: &Path,
    deadline: Instant,
    limits: LocalTreeCertificationLimits,
) -> io::Result<()> {
    require_local_tree_with_exclusions_and_limits_until(path, &[], &[], false, deadline, limits)
}

fn require_local_tree_with_exclusions_and_limits_until(
    path: &Path,
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    filter_go_ignored_directories: bool,
    deadline: Instant,
    limits: LocalTreeCertificationLimits,
) -> io::Result<()> {
    const OPERATION: &str = "Windows local tree certification";

    require_file_io_before_deadline(deadline, OPERATION)?;
    validate_windows_path_units(path)?;
    validate_scoped_path_batch(exclusions, deadline, OPERATION)?;
    validate_scoped_path_batch(inclusions, deadline, OPERATION)?;
    let root = path.to_path_buf();
    let mut owned_exclusions = Vec::new();
    owned_exclusions
        .try_reserve_exact(exclusions.len())
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("could not allocate bounded Windows exclusions: {error}"),
            )
        })?;
    for exclusion in exclusions {
        require_file_io_before_deadline(deadline, OPERATION)?;
        owned_exclusions.push(exclusion.clone());
    }
    let mut owned_inclusions = Vec::new();
    owned_inclusions
        .try_reserve_exact(inclusions.len())
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("could not allocate bounded Windows inclusions: {error}"),
            )
        })?;
    for inclusion in inclusions {
        require_file_io_before_deadline(deadline, OPERATION)?;
        owned_inclusions.push(inclusion.clone());
    }
    require_file_io_before_deadline(deadline, OPERATION)?;
    run_cancellable_file_io_pass(deadline, OPERATION, move || {
        require_local_tree_inner(
            &root,
            &owned_exclusions,
            &owned_inclusions,
            filter_go_ignored_directories,
            deadline,
            limits,
        )
        .map(|_| ())
    })?
}

fn require_local_tree_inner(
    path: &Path,
    exclusions: &[PathBuf],
    inclusions: &[PathBuf],
    filter_go_ignored_directories: bool,
    deadline: Instant,
    limits: LocalTreeCertificationLimits,
) -> io::Result<CertifiedLocalTree> {
    const OPERATION: &str = "Windows local tree certification";

    require_file_io_before_deadline(deadline, OPERATION)?;
    validate_windows_path_units(path)?;
    let exclusions = ScopedPathIndex::new(exclusions, deadline, OPERATION)?;
    let inclusions = ScopedPathIndex::new(inclusions, deadline, OPERATION)?;
    require_file_io_before_deadline(deadline, OPERATION)?;
    let certified = certified_local_directory_inner(path, deadline)?;
    let root = certified.root.clone();
    for exclusion in &exclusions.paths {
        require_file_io_before_deadline(deadline, OPERATION)?;
        if exclusion == &root || !exclusion.starts_with(&root) {
            return Err(invalid_data(
                exclusion,
                "a local-tree exclusion must be a proper descendant of the certified root",
            ));
        }
        require_local_fixed_volume(exclusion)?;
        let metadata = std::fs::symlink_metadata(exclusion)?;
        if is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(invalid_data(
                exclusion,
                "a local-tree exclusion boundary must be a direct directory",
            ));
        }
    }
    let mut collapsed_exclusions = Vec::new();
    collapsed_exclusions
        .try_reserve_exact(exclusions.paths.len())
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("could not allocate bounded collapsed Windows exclusions: {error}"),
            )
        })?;
    for exclusion in exclusions.paths {
        require_file_io_before_deadline(deadline, OPERATION)?;
        if collapsed_exclusions
            .last()
            .is_some_and(|parent: &PathBuf| exclusion.starts_with(parent))
        {
            continue;
        }
        collapsed_exclusions.push(exclusion);
    }
    let collapsed_exclusions = ScopedPathIndex {
        paths: collapsed_exclusions,
    };
    for inclusion in &inclusions.paths {
        require_file_io_before_deadline(deadline, OPERATION)?;
        if !inclusion.starts_with(&root) {
            return Err(invalid_data(
                inclusion,
                "a local-tree inclusion must be within the certified root",
            ));
        }
        if collapsed_exclusions.contains_ancestor_of(inclusion, &root) {
            return Err(invalid_data(
                inclusion,
                "a local-tree inclusion must not overlap an excluded tree",
            ));
        }
        require_local_fixed_volume(inclusion)?;
        let metadata = std::fs::symlink_metadata(inclusion)?;
        if is_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(invalid_data(
                inclusion,
                "a local-tree inclusion must be a direct directory",
            ));
        }
    }
    if limits.frontier == 0 {
        return Err(local_tree_limit_error(
            &root,
            "pending-directory frontier",
            limits.frontier,
        ));
    }

    let root_path_units = root.as_os_str().encode_wide().count();
    if root_path_units > limits.frontier_path_units {
        return Err(local_tree_limit_error(
            &root,
            "pending-directory path units",
            limits.frontier_path_units,
        ));
    }
    let mut frontier = vec![(root.clone(), 0_usize, root_path_units)];
    let mut frontier_path_units = root_path_units;
    let mut entries = 0_usize;
    while let Some((directory, depth, directory_path_units)) = frontier.pop() {
        frontier_path_units = frontier_path_units
            .checked_sub(directory_path_units)
            .ok_or_else(|| invalid_data(&root, "pending-directory path accounting underflow"))?;
        require_file_io_before_deadline(deadline, OPERATION)?;
        let directory_metadata = std::fs::symlink_metadata(&directory)?;
        if is_reparse_point(&directory_metadata) || !directory_metadata.is_dir() {
            return Err(invalid_data(
                &directory,
                "a recursively inspected directory changed type during certification",
            ));
        }
        let children = std::fs::read_dir(&directory)?;
        for child in children {
            require_file_io_before_deadline(deadline, OPERATION)?;
            let child = child?;
            let child_path = child.path();
            let cache_boundary_or_ancestor =
                collapsed_exclusions.contains_descendant_of(&child_path);
            let selected_boundary_or_ancestor = inclusions.contains_descendant_of(&child_path);
            entries = entries
                .checked_add(1)
                .ok_or_else(|| local_tree_limit_error(&root, "entry count", limits.entries))?;
            if entries > limits.entries {
                return Err(local_tree_limit_error(&root, "entry count", limits.entries));
            }
            if filter_go_ignored_directories
                && windows_go_ignored_directory_name(&child.file_name())
                && !cache_boundary_or_ancestor
                && !selected_boundary_or_ancestor
            {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&child_path)?;
            if is_reparse_point(&metadata) {
                return Err(invalid_data(
                    &child_path,
                    "a recursively inspected tree must not contain reparse points",
                ));
            }
            if metadata.is_file() {
                continue;
            }
            if collapsed_exclusions.contains_exact(&child_path) {
                if !metadata.is_dir() {
                    return Err(invalid_data(
                        &child_path,
                        "a local-tree exclusion boundary changed type during certification",
                    ));
                }
                continue;
            }
            if metadata.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    local_tree_limit_error(&root, "directory depth", limits.depth)
                })?;
                if child_depth > limits.depth {
                    return Err(local_tree_limit_error(
                        &root,
                        "directory depth",
                        limits.depth,
                    ));
                }
                let child_path_units = child_path.as_os_str().encode_wide().count();
                let next_frontier_path_units = frontier_path_units
                    .checked_add(child_path_units)
                    .ok_or_else(|| {
                        local_tree_limit_error(
                            &root,
                            "pending-directory path units",
                            limits.frontier_path_units,
                        )
                    })?;
                if next_frontier_path_units > limits.frontier_path_units {
                    return Err(local_tree_limit_error(
                        &root,
                        "pending-directory path units",
                        limits.frontier_path_units,
                    ));
                }
                frontier.try_reserve(1).map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("could not allocate the Windows local-tree frontier: {error}"),
                    )
                })?;
                frontier.push((child_path, child_depth, child_path_units));
                frontier_path_units = next_frontier_path_units;
                if frontier.len() > limits.frontier {
                    return Err(local_tree_limit_error(
                        &root,
                        "pending-directory frontier",
                        limits.frontier,
                    ));
                }
            } else {
                return Err(invalid_data(
                    &child_path,
                    "a recursively inspected tree must contain only files and directories",
                ));
            }
        }
    }
    require_file_io_before_deadline(deadline, OPERATION)?;
    certified.verify_root_binding()?;
    Ok(certified)
}

fn certified_local_directory_inner(
    path: &Path,
    deadline: Instant,
) -> io::Result<CertifiedLocalTree> {
    const OPERATION: &str = "Windows local directory certification";

    require_file_io_before_deadline(deadline, OPERATION)?;
    let root = scoped_absolute_path(path)?;
    require_local_fixed_volume(&root)?;
    require_file_io_before_deadline(deadline, OPERATION)?;
    let root_handle = file_from_handle(open_existing_path(
        &root,
        FILE_READ_ATTRIBUTES | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    )?);
    let root_identity = handle_identity(file_handle(&root_handle))?;
    reject_reparse_identity(&root, root_identity)?;
    if !root_identity.directory {
        return Err(invalid_data(&root, "the certified root is not a directory"));
    }
    let root_final_path = final_path(file_handle(&root_handle))?;
    Ok(CertifiedLocalTree {
        root,
        root_handle,
        root_identity,
        root_final_path,
        owner_thread: thread::current().id(),
        deadline,
    })
}

fn local_tree_limit_error(path: &Path, dimension: &str, limit: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "unsafe Windows path `{}`: local tree {dimension} exceeds its certification limit of {limit}",
            path.display()
        ),
    )
}

fn windows_go_ignored_directory_name(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name == "testdata" || name.starts_with('.') || name.starts_with('_')
}

/// Creates one owner-only directory without following or inheriting a mutable
/// security boundary. The parent must already exist and be certified local.
#[allow(
    unsafe_code,
    reason = "CreateDirectoryW receives a live path and owned private security descriptor"
)]
pub(super) fn create_private_directory(path: &Path) -> io::Result<()> {
    require_private_creation_parent(path)?;
    let path_wide = wide_path(path)?;
    let mut security = PrivateSecurityDescriptor::new(true, false)?;
    let attributes = security.attributes()?;
    // SAFETY: `path_wide` is NUL-terminated and the security descriptor, ACL,
    // and SID backing storage all outlive the call. The handle is explicitly
    // non-inheritable.
    if unsafe { CreateDirectoryW(path_wide.as_ptr(), &raw const attributes) } == 0 {
        let error = last_error();
        if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
            return Err(error);
        }
        make_private_path_writable(path, true)?;
    }
    verify_private_path(path, true, false)
}

/// Creates a new owner-only file, flushes its contents, and optionally seals it
/// before releasing the replacement-blocking handle.
#[allow(
    unsafe_code,
    reason = "CreateFileW returns a fresh handle using a live private security descriptor"
)]
pub(super) fn create_private_file(path: &Path, bytes: &[u8], sealed: bool) -> io::Result<()> {
    require_private_creation_parent(path)?;
    let mut security = PrivateSecurityDescriptor::new(false, false)?;
    let attributes = security.attributes()?;
    let path_wide = wide_path(path)?;
    // SAFETY: the path and security backing storage remain live for the call;
    // CREATE_NEW returns a unique handle or fails without replacing anything.
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_READ
                | GENERIC_WRITE
                | READ_CONTROL
                | WRITE_DAC
                | WRITE_OWNER
                | FILE_READ_ATTRIBUTES
                | FILE_WRITE_ATTRIBUTES,
            0,
            &raw const attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    // SAFETY: CreateFileW returned a fresh uniquely-owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
    let mut file = file_from_handle(handle);
    verify_private_handle(file_handle(&file), false, false)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if sealed {
        set_handle_private_access(file_handle(&file), false, true)?;
    } else {
        verify_private_handle(file_handle(&file), false, false)?;
    }
    drop(file);
    verify_private_path(path, false, sealed)
}

/// Creates a new owner-only regular file suitable for an OS file lock.
pub(super) fn create_private_lock_file(path: &Path) -> io::Result<File> {
    open_private_lock_file_with_disposition(path, CREATE_NEW)
}

/// Opens or creates an owner-only regular file suitable for an OS file lock.
pub(super) fn open_private_lock_file(path: &Path) -> io::Result<File> {
    open_private_lock_file_with_disposition(path, OPEN_ALWAYS)
}

#[allow(
    unsafe_code,
    reason = "CreateFileW atomically creates or opens a non-inheritable private lock handle"
)]
fn open_private_lock_file_with_disposition(path: &Path, disposition: u32) -> io::Result<File> {
    require_private_creation_parent(path)?;
    let path_wide = wide_path(path)?;
    let mut security = PrivateSecurityDescriptor::new(false, false)?;
    let attributes = security.attributes()?;
    // SAFETY: the path is NUL-terminated and the security descriptor remains
    // live for the call. The caller-selected disposition performs creation or
    // collision handling in this single kernel operation.
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_READ
                | GENERIC_WRITE
                | READ_CONTROL
                | FILE_READ_ATTRIBUTES
                | FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &raw const attributes,
            disposition,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    // SAFETY: CreateFileW returned a fresh uniquely-owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
    let identity = handle_identity(handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, identity)?;
    if identity.directory {
        return Err(invalid_data(path, "expected a regular lock file"));
    }
    verify_private_handle(handle.as_raw_handle().cast(), false, false)?;
    Ok(file_from_handle(handle))
}

pub(super) fn open_existing_private_lock_file(path: &Path) -> io::Result<File> {
    let handle = open_existing_path(
        path,
        GENERIC_READ | GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    let identity = handle_identity(handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, identity)?;
    if identity.directory {
        return Err(invalid_data(path, "expected a regular lock file"));
    }
    verify_private_handle(handle.as_raw_handle().cast(), false, false)?;
    Ok(file_from_handle(handle))
}

/// Rewrites an existing mutable owner-only file through a stable no-follow
/// handle. The file is never truncated until its identity and DACL pass.
pub(super) fn overwrite_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    require_local_fixed_volume(path)?;
    let mut file = file_from_handle(open_existing_path(
        path,
        GENERIC_READ
            | GENERIC_WRITE
            | READ_CONTROL
            | WRITE_DAC
            | FILE_READ_ATTRIBUTES
            | FILE_WRITE_ATTRIBUTES,
        0,
        FILE_FLAG_OPEN_REPARSE_POINT,
    )?);
    let identity = handle_identity(file_handle(&file))?;
    reject_reparse_identity(path, identity)?;
    if identity.directory {
        return Err(invalid_data(
            path,
            "expected a regular file, found a directory",
        ));
    }
    verify_private_handle(file_handle(&file), false, false)?;
    file.set_len(0)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    verify_private_handle(file_handle(&file), false, false)?;
    drop(file);
    verify_private_path(path, false, false)
}

pub(super) fn seal_private_path(path: &Path, directory: bool) -> io::Result<()> {
    set_private_path_access(path, directory, true)
}

pub(super) fn make_private_path_writable(path: &Path, directory: bool) -> io::Result<()> {
    set_private_path_access(path, directory, false)
}

pub(super) fn verify_private_path(path: &Path, directory: bool, sealed: bool) -> io::Result<()> {
    require_local_fixed_volume(path)?;
    verify_private_path_in_certified_tree(path, directory, sealed)
}

pub(super) fn verify_private_path_in_certified_tree(
    path: &Path,
    directory: bool,
    sealed: bool,
) -> io::Result<()> {
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    let handle = open_existing_path(
        path,
        FILE_READ_ATTRIBUTES | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        flags,
    )?;
    let identity = handle_identity(handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, identity)?;
    if identity.directory != directory {
        return Err(invalid_data(
            path,
            "the entry type changed during verification",
        ));
    }
    verify_private_handle(handle.as_raw_handle().cast(), directory, sealed)
}

pub(super) fn identity_no_follow(path: &Path) -> io::Result<WindowsFileIdentity> {
    require_local_fixed_volume(path)?;
    identity_no_follow_in_certified_tree(path)
}

pub(super) fn identity_no_follow_in_certified_tree(path: &Path) -> io::Result<WindowsFileIdentity> {
    let handle = open_existing_path(
        path,
        FILE_READ_ATTRIBUTES | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    let identity = handle_identity(handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, identity)?;
    Ok(identity)
}

#[allow(
    unsafe_code,
    reason = "UrlCreateFromPathW receives bounded live UTF-16 input and output buffers"
)]
pub(super) fn file_url(path: &Path) -> io::Result<String> {
    require_local_fixed_volume(path)?;
    let canonical = std::fs::canonicalize(path)?;
    let canonical_units = canonical.as_os_str().encode_wide().collect::<Vec<_>>();
    let extended_prefix = [
        u16::from(b'\\'),
        u16::from(b'\\'),
        u16::from(b'?'),
        u16::from(b'\\'),
    ];
    let dos_units = canonical_units
        .strip_prefix(&extended_prefix)
        .unwrap_or(&canonical_units);
    if dos_units.starts_with(&[
        u16::from(b'U'),
        u16::from(b'N'),
        u16::from(b'C'),
        u16::from(b'\\'),
    ]) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "UNC paths cannot be used as a local Go module proxy",
        ));
    }
    let mut input = dos_units.to_vec();
    input.push(0);
    let mut output = vec![0_u16; MAX_WINDOWS_PATH_UNITS.saturating_mul(3)];
    let mut output_length = u32::try_from(output.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows file URL buffer is too large",
        )
    })?;
    // SAFETY: both buffers remain live, input is NUL-terminated, and
    // `output_length` advertises the exact writable UTF-16 capacity.
    let result = unsafe {
        UrlCreateFromPathW(
            input.as_ptr(),
            output.as_mut_ptr(),
            &raw mut output_length,
            0,
        )
    };
    if result < 0 {
        return Err(io::Error::other(format!(
            "UrlCreateFromPathW failed with HRESULT 0x{:08X}",
            result as u32
        )));
    }
    let length = usize::try_from(output_length).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned an oversized file URL",
        )
    })?;
    if length > output.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned an oversized file URL",
        ));
    }
    output.truncate(length);
    if output.last() == Some(&0) {
        output.pop();
    }
    String::from_utf16(&output).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows returned an invalid UTF-16 file URL",
        )
    })
}

fn set_private_path_access(path: &Path, directory: bool, sealed: bool) -> io::Result<()> {
    require_local_fixed_volume(path)?;
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    // A sealed DACL deliberately omits FILE_WRITE_ATTRIBUTES and WRITE_OWNER.
    // Open the replacement-blocking control handle with rights every owner
    // retains, then obtain the extra transition rights only when the token's
    // default owner must be normalized or a mutable file must be sealed.
    let control_access = FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC;
    let handle = open_existing_path(
        path,
        control_access,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        flags,
    )?;
    let identity = handle_identity(handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, identity)?;
    if identity.directory != directory {
        return Err(invalid_data(
            path,
            "the entry type changed while changing its DACL",
        ));
    }
    let normalize_owner =
        current_token_owner_requires_normalization(handle.as_raw_handle().cast())?;
    let handle = if normalize_owner {
        let owner_access = control_access
            | WRITE_OWNER
            | if !directory && sealed {
                FILE_WRITE_ATTRIBUTES
            } else {
                0
            };
        let owner_handle = open_existing_path(
            path,
            owner_access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            flags,
        )?;
        let owner_identity = handle_identity(owner_handle.as_raw_handle().cast())?;
        reject_reparse_identity(path, owner_identity)?;
        if !identity.names_same_object(owner_identity) {
            return Err(invalid_data(
                path,
                "the entry identity changed while normalizing its owner",
            ));
        }
        owner_handle
    } else {
        handle
    };
    if directory {
        return set_handle_private_access(handle.as_raw_handle().cast(), directory, sealed);
    }

    if sealed {
        if normalize_owner {
            return set_handle_private_access(handle.as_raw_handle().cast(), false, true);
        }
        if verify_private_handle(handle.as_raw_handle().cast(), false, true).is_ok() {
            return Ok(());
        }
        verify_private_handle(handle.as_raw_handle().cast(), false, false)?;
        let attribute_handle = open_existing_path(
            path,
            FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES | READ_CONTROL | WRITE_DAC,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            flags,
        )?;
        let attribute_identity = handle_identity(attribute_handle.as_raw_handle().cast())?;
        reject_reparse_identity(path, attribute_identity)?;
        if !identity.names_same_object(attribute_identity) {
            return Err(invalid_data(
                path,
                "the file identity changed while sealing it",
            ));
        }
        verify_private_handle(attribute_handle.as_raw_handle().cast(), false, false)?;
        return set_handle_private_access(attribute_handle.as_raw_handle().cast(), false, true);
    }

    set_handle_private_dacl(handle.as_raw_handle().cast(), false, false)?;
    let attribute_handle = open_existing_path(
        path,
        FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        flags,
    )?;
    let attribute_identity = handle_identity(attribute_handle.as_raw_handle().cast())?;
    reject_reparse_identity(path, attribute_identity)?;
    if !identity.names_same_object(attribute_identity) {
        return Err(invalid_data(
            path,
            "the file identity changed while making it writable",
        ));
    }
    verify_private_handle(attribute_handle.as_raw_handle().cast(), false, false)?;
    set_handle_readonly(attribute_handle.as_raw_handle().cast(), false)?;
    verify_private_handle(attribute_handle.as_raw_handle().cast(), false, false)
}

fn require_private_creation_parent(path: &Path) -> io::Result<()> {
    reject_alternate_data_streams(path)?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a private Windows path must have an existing parent",
        )
    })?;
    require_local_fixed_volume(parent)
}

struct CurrentTokenSid {
    _storage: Vec<usize>,
    sid: PSID,
}

impl CurrentTokenSid {
    fn user() -> io::Result<Self> {
        current_token_sid(CurrentTokenSidKind::User)
    }

    fn default_owner() -> io::Result<Self> {
        current_token_sid(CurrentTokenSidKind::DefaultOwner)
    }
}

#[derive(Clone, Copy)]
enum CurrentTokenSidKind {
    User,
    DefaultOwner,
}

struct PrivateSecurityDescriptor {
    _owner: CurrentTokenSid,
    _acl: Vec<u32>,
    descriptor: Box<SECURITY_DESCRIPTOR>,
}

impl PrivateSecurityDescriptor {
    fn new(directory: bool, sealed: bool) -> io::Result<Self> {
        build_private_security_descriptor(directory, sealed)
    }

    fn attributes(&mut self) -> io::Result<SECURITY_ATTRIBUTES> {
        Ok(SECURITY_ATTRIBUTES {
            nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows security attributes are too large",
                )
            })?,
            lpSecurityDescriptor: self.descriptor.as_mut() as *mut SECURITY_DESCRIPTOR
                as *mut core::ffi::c_void,
            bInheritHandle: 0,
        })
    }
}

struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);

impl Drop for LocalSecurityDescriptor {
    #[allow(
        unsafe_code,
        reason = "LocalFree releases the descriptor allocated by GetSecurityInfo"
    )]
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: GetSecurityInfo returned this descriptor via LocalAlloc,
            // and this RAII owner releases it exactly once.
            unsafe {
                LocalFree(self.0.cast());
            }
        }
    }
}

#[allow(
    unsafe_code,
    reason = "token APIs fill an aligned owned buffer whose embedded SID remains live"
)]
fn current_token_sid(kind: CurrentTokenSidKind) -> io::Result<CurrentTokenSid> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: the current-process pseudo-handle is always valid and `token` is
    // a writable output for a fresh non-inheritable handle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(last_error());
    }
    // SAFETY: OpenProcessToken returned a fresh uniquely-owned handle.
    let token = unsafe { OwnedHandle::from_raw_handle(token.cast()) };
    let mut required = 0_u32;
    let information_class = match kind {
        CurrentTokenSidKind::User => TokenUser,
        CurrentTokenSidKind::DefaultOwner => TokenOwner,
    };
    let label = match kind {
        CurrentTokenSidKind::User => "user",
        CurrentTokenSidKind::DefaultOwner => "default-owner",
    };
    // SAFETY: a null buffer with length zero is the documented sizing query.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle().cast(),
            information_class,
            std::ptr::null_mut(),
            0,
            &raw mut required,
        )
    } != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Windows unexpectedly accepted an empty token-{label} buffer"),
        ));
    }
    let sizing_error = last_error();
    if sizing_error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) || required == 0 {
        return Err(sizing_error);
    }
    let required_usize = usize::try_from(required).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("token-{label} buffer is too large"),
        )
    })?;
    let words = required_usize
        .checked_add(std::mem::size_of::<usize>() - 1)
        .and_then(|value| value.checked_div(std::mem::size_of::<usize>()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("token-{label} buffer is too large"),
            )
        })?;
    let mut storage = vec![0_usize; words];
    // SAFETY: the aligned allocation is writable for at least `required`
    // bytes, and the token handle remains live.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle().cast(),
            information_class,
            storage.as_mut_ptr().cast(),
            required,
            &raw mut required,
        )
    } == 0
    {
        return Err(last_error());
    }
    // SAFETY: the successful query initialized the selected token-information
    // structure at the beginning of the aligned buffer.
    let sid = unsafe {
        match kind {
            CurrentTokenSidKind::User => (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid,
            CurrentTokenSidKind::DefaultOwner => (*(storage.as_ptr().cast::<TOKEN_OWNER>())).Owner,
        }
    };
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Windows returned an invalid current token {label} SID"),
        ));
    }
    Ok(CurrentTokenSid {
        _storage: storage,
        sid,
    })
}

#[allow(
    unsafe_code,
    reason = "ACL APIs initialize owned aligned storage and an owned absolute descriptor"
)]
fn build_private_security_descriptor(
    directory: bool,
    sealed: bool,
) -> io::Result<PrivateSecurityDescriptor> {
    let owner = CurrentTokenSid::user()?;
    // SAFETY: `owner.sid` is valid for the lifetime of `owner`.
    let sid_length = unsafe { GetLengthSid(owner.sid) };
    if sid_length == 0 {
        return Err(last_error());
    }
    let acl_bytes = std::mem::size_of::<ACL>()
        .checked_add(std::mem::size_of::<ACCESS_ALLOWED_ACE>() - std::mem::size_of::<u32>())
        .and_then(|value| value.checked_add(usize::try_from(sid_length).ok()?))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "private ACL is too large"))?;
    let acl_words = acl_bytes
        .checked_add(std::mem::size_of::<u32>() - 1)
        .and_then(|value| value.checked_div(std::mem::size_of::<u32>()))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "private ACL is too large"))?;
    let mut acl = vec![0_u32; acl_words];
    let acl_length = u32::try_from(acl.len() * std::mem::size_of::<u32>())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "private ACL is too large"))?;
    let acl_ptr = acl.as_mut_ptr().cast::<ACL>();
    // SAFETY: `acl` is aligned, writable for `acl_length`, and remains owned
    // by the returned descriptor.
    if unsafe { InitializeAcl(acl_ptr, acl_length, ACL_REVISION) } == 0 {
        return Err(last_error());
    }
    let ace_flags = if directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    };
    let access_mask = private_access_mask(sealed);
    // SAFETY: the initialized ACL has exactly enough capacity for this ACE and
    // valid SID, both of which remain live.
    if unsafe { AddAccessAllowedAceEx(acl_ptr, ACL_REVISION, ace_flags, access_mask, owner.sid) }
        == 0
    {
        return Err(last_error());
    }
    let mut descriptor = Box::<SECURITY_DESCRIPTOR>::default();
    let descriptor_ptr = descriptor.as_mut() as *mut SECURITY_DESCRIPTOR as PSECURITY_DESCRIPTOR;
    // SAFETY: `descriptor` is writable and each referenced SID/ACL remains live
    // in the returned aggregate.
    if unsafe { InitializeSecurityDescriptor(descriptor_ptr, SECURITY_DESCRIPTOR_REVISION) } == 0
        || unsafe { SetSecurityDescriptorOwner(descriptor_ptr, owner.sid, 0) } == 0
        || unsafe { SetSecurityDescriptorDacl(descriptor_ptr, 1, acl_ptr, 0) } == 0
        || unsafe {
            SetSecurityDescriptorControl(descriptor_ptr, SE_DACL_PROTECTED, SE_DACL_PROTECTED)
        } == 0
    {
        return Err(last_error());
    }
    Ok(PrivateSecurityDescriptor {
        _owner: owner,
        _acl: acl,
        descriptor,
    })
}

fn private_access_mask(sealed: bool) -> u32 {
    if sealed {
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
    } else {
        FILE_ALL_ACCESS
    }
}

#[allow(
    unsafe_code,
    reason = "security APIs inspect a kernel-validated descriptor returned for a live handle"
)]
fn private_handle_access(handle: HANDLE, directory: bool) -> io::Result<WindowsPrivateAccess> {
    let current_user = CurrentTokenSid::user()?;
    let mut owner: PSID = std::ptr::null_mut();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: all outputs are writable and the queried handle remains live.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            std::ptr::null_mut(),
            &raw mut dacl,
            std::ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    if descriptor.0.is_null() || owner.is_null() || dacl.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry has an incomplete security descriptor",
        ));
    }
    // SAFETY: both SIDs were validated by Windows and remain live.
    if unsafe { EqualSid(owner, current_user.sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry is not owned by the current user",
        ));
    }
    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: the descriptor remains owned by `descriptor` and both outputs
    // are writable.
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &raw mut control, &raw mut revision) }
        == 0
    {
        return Err(last_error());
    }
    if control & SE_DACL_PROTECTED == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry inherits access-control entries",
        ));
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    let size_length = u32::try_from(std::mem::size_of_val(&size))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "ACL size query is too large"))?;
    // SAFETY: `dacl` belongs to the live descriptor and `size` is writable for
    // its advertised size.
    if unsafe {
        GetAclInformation(
            dacl,
            (&raw mut size).cast(),
            size_length,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(last_error());
    }
    if size.AceCount != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry must have exactly one access-control entry",
        ));
    }
    let mut ace_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: the ACL reports exactly one entry and `ace_ptr` is writable.
    if unsafe { GetAce(dacl, 0, &raw mut ace_ptr) } == 0 || ace_ptr.is_null() {
        return Err(last_error());
    }
    // SAFETY: GetAce returned a pointer to a kernel-validated ACE in the live
    // descriptor. ACCESS_ALLOWED_ACE begins with the common ACE header.
    let ace = unsafe { &*ace_ptr.cast::<ACCESS_ALLOWED_ACE>() };
    let expected_flags = u8::try_from(if directory {
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    } else {
        0
    })
    .expect("private ACE flags fit in a byte");
    if ace.Header.AceType != ACCESS_ALLOWED_ACE_TYPE || ace.Header.AceFlags != expected_flags {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry has unexpected access-control rights",
        ));
    }
    let access = if ace.Mask == private_access_mask(false) {
        WindowsPrivateAccess::Mutable
    } else if ace.Mask == private_access_mask(true) {
        WindowsPrivateAccess::Sealed
    } else {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry has unexpected access-control rights",
        ));
    };
    let ace_sid = (&raw const ace.SidStart)
        .cast_mut()
        .cast::<core::ffi::c_void>();
    // SAFETY: SidStart is the documented start of the variable-length SID in
    // ACCESS_ALLOWED_ACE, and the ACL remains live.
    if unsafe { IsValidSid(ace_sid) } == 0 || unsafe { EqualSid(ace_sid, current_user.sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry grants access to an unexpected principal",
        ));
    }
    Ok(access)
}

fn verify_private_handle(handle: HANDLE, directory: bool, sealed: bool) -> io::Result<()> {
    let actual = private_handle_access(handle, directory)?;
    let expected = if sealed {
        WindowsPrivateAccess::Sealed
    } else {
        WindowsPrivateAccess::Mutable
    };
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private Windows entry has unexpected access-control rights",
        ));
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "SetSecurityInfo and handle metadata updates operate on one live replacement-blocking handle"
)]
fn set_handle_private_access(handle: HANDLE, directory: bool, sealed: bool) -> io::Result<()> {
    if !directory && sealed {
        set_handle_readonly(handle, true)?;
    }
    set_handle_private_dacl(handle, directory, sealed)?;
    if !directory && !sealed {
        set_handle_readonly(handle, false)?;
    }
    verify_private_handle(handle, directory, sealed)
}

#[allow(
    unsafe_code,
    reason = "SetSecurityInfo replaces the DACL on one live replacement-blocking handle"
)]
fn set_handle_private_dacl(handle: HANDLE, directory: bool, sealed: bool) -> io::Result<()> {
    // Refuse to rewrite another principal's entry even if the caller somehow
    // has control rights through a broader inherited entry. Fresh filesystem
    // objects may initially use the token's default owner, so normalize that
    // owner to TokenUser as part of the same protected-DACL transition.
    let normalize_owner = current_token_owner_requires_normalization(handle)?;
    let security = PrivateSecurityDescriptor::new(directory, sealed)?;
    let acl = security._acl.as_ptr().cast::<ACL>();
    let owner = if normalize_owner {
        security._owner.sid
    } else {
        std::ptr::null_mut()
    };
    let security_information = DACL_SECURITY_INFORMATION
        | PROTECTED_DACL_SECURITY_INFORMATION
        | if normalize_owner {
            OWNER_SECURITY_INFORMATION
        } else {
            0
        };
    // SAFETY: the handle remains live and the initialized owner SID and ACL
    // remain owned by `security` for the full call.
    let status = unsafe {
        SetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            security_information,
            owner,
            std::ptr::null_mut(),
            acl,
            std::ptr::null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    verify_private_handle(handle, directory, sealed)
}

#[allow(
    unsafe_code,
    reason = "GetSecurityInfo returns an owned descriptor used only for an owner SID comparison"
)]
fn current_token_owner_requires_normalization(handle: HANDLE) -> io::Result<bool> {
    let current_user = CurrentTokenSid::user()?;
    let mut owner: PSID = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: outputs are writable and the handle remains live.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    if descriptor.0.is_null() || owner.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to change a Windows entry not owned by the current user",
        ));
    }
    // SAFETY: both SIDs remain live for this comparison.
    if unsafe { EqualSid(owner, current_user.sid) } != 0 {
        return Ok(false);
    }
    let default_owner = CurrentTokenSid::default_owner()?;
    // SAFETY: both SIDs remain live for this comparison.
    if unsafe { EqualSid(owner, default_owner.sid) } != 0 {
        return Ok(true);
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "refusing to change a Windows entry not owned by the current token",
    ))
}

#[allow(
    unsafe_code,
    reason = "SetFileInformationByHandle updates attributes on the already-pinned file"
)]
fn set_handle_readonly(handle: HANDLE, readonly: bool) -> io::Result<()> {
    let mut basic = query_handle_information::<FILE_BASIC_INFO>(handle, FileBasicInfo)?;
    if readonly {
        basic.FileAttributes |= FILE_ATTRIBUTE_READONLY;
    } else {
        basic.FileAttributes &= !FILE_ATTRIBUTE_READONLY;
    }
    let length = u32::try_from(std::mem::size_of_val(&basic)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows basic file information is too large",
        )
    })?;
    // SAFETY: `basic` matches FileBasicInfo and remains live for the call.
    if unsafe {
        SetFileInformationByHandle(handle, FileBasicInfo, (&raw const basic).cast(), length)
    } == 0
    {
        return Err(last_error());
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(super) struct KillOnCloseJob {
    handle: Arc<OwnedHandle>,
}

impl KillOnCloseJob {
    pub(super) fn new() -> io::Result<Self> {
        let handle = create_kill_on_close_job()?;
        Ok(Self {
            handle: Arc::new(handle),
        })
    }

    pub(super) fn assign_suspended_child(
        &self,
        child: &Child,
        deadline: Instant,
    ) -> io::Result<SuspendedChild> {
        let thread = sole_process_thread(child.id(), deadline)?;
        assign_process_to_job(self.raw_handle(), child.as_raw_handle().cast())?;
        Ok(SuspendedChild {
            job: self.clone(),
            thread: Some(thread),
        })
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        terminate_job(self.raw_handle())
    }

    fn raw_handle(&self) -> HANDLE {
        self.handle.as_raw_handle().cast()
    }
}

pub(super) fn configure_suspended_command(command: &mut Command) {
    command.creation_flags(CREATE_SUSPENDED);
}

#[derive(Debug)]
pub(super) struct SuspendedChild {
    job: KillOnCloseJob,
    thread: Option<OwnedHandle>,
}

impl SuspendedChild {
    pub(super) fn resume(mut self) -> io::Result<()> {
        let thread = self.thread.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "the child was already resumed")
        })?;
        resume_thread(thread.as_raw_handle().cast())?;
        Ok(())
    }
}

impl Drop for SuspendedChild {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.job.terminate();
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ThreadIoCanceller {
    thread: Arc<OwnedHandle>,
}

impl ThreadIoCanceller {
    pub(super) fn for_current_thread() -> io::Result<Self> {
        Ok(Self {
            thread: Arc::new(duplicate_current_thread_handle()?),
        })
    }

    pub(super) fn cancel(&self) -> io::Result<()> {
        cancel_synchronous_io(self.thread.as_raw_handle().cast())
    }
}

fn run_cancellable_file_io<T: Send + 'static>(
    deadline: Instant,
    operation: &'static str,
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    run_cancellable_file_io_pass(deadline, operation, work)?
}

/// Runs an owned filesystem certification pass on one cancellable worker.
///
/// Secure-file reads and hashes invoked by `work` execute inline on that
/// worker. The caller therefore pays for one thread and one duplicated handle
/// per pass, while `CancelSynchronousIo` can still interrupt whichever file is
/// blocked when the absolute deadline expires.
pub(super) fn run_cancellable_file_io_pass<T: Send + 'static>(
    deadline: Instant,
    operation: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> io::Result<T> {
    require_file_io_before_deadline(deadline, operation)?;
    if cancellable_file_io_pass_is_active() {
        let result = work();
        require_file_io_before_deadline(deadline, operation)?;
        return Ok(result);
    }
    let (setup_sender, setup_receiver) = mpsc::sync_channel(1);
    let (start_sender, start_receiver) = mpsc::sync_channel(1);
    let (result_sender, result_receiver) = mpsc::sync_channel(1);
    let worker = thread::Builder::new()
        .name("polint-windows-file-io".to_string())
        .spawn(move || {
            let canceller = ThreadIoCanceller::for_current_thread();
            let setup_succeeded = canceller.is_ok();
            if setup_sender.send(canceller).is_err() || !setup_succeeded {
                return;
            }
            if start_receiver.recv().is_err() {
                return;
            }
            let _pass = CancellableFileIoPassGuard::enter();
            let _ = result_sender.send(work());
        })?;
    #[cfg(test)]
    FILE_IO_WORKER_SPAWNS.set(FILE_IO_WORKER_SPAWNS.get().saturating_add(1));

    let remaining = deadline.saturating_duration_since(Instant::now());
    let canceller = match setup_receiver.recv_timeout(remaining) {
        Ok(Ok(canceller)) => canceller,
        Ok(Err(error)) => {
            drop(start_sender);
            let _ = worker.join();
            return Err(error);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            drop(start_sender);
            drop(worker);
            return Err(file_io_timeout(operation, None));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            drop(start_sender);
            return match worker.join() {
                Ok(()) => Err(io::Error::other(format!(
                    "{operation} worker stopped during cancellation setup"
                ))),
                Err(_) => Err(io::Error::other(format!("{operation} worker panicked"))),
            };
        }
    };
    require_file_io_before_deadline(deadline, operation)?;
    start_sender
        .send(())
        .map_err(|_| io::Error::other(format!("{operation} worker stopped before starting I/O")))?;

    let remaining = deadline.saturating_duration_since(Instant::now());
    match result_receiver.recv_timeout(remaining) {
        Ok(result) => {
            worker
                .join()
                .map_err(|_| io::Error::other(format!("{operation} worker panicked")))?;
            require_file_io_before_deadline(deadline, operation)?;
            Ok(result)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
            Ok(()) => Err(io::Error::other(format!(
                "{operation} worker stopped without a result"
            ))),
            Err(_) => Err(io::Error::other(format!("{operation} worker panicked"))),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let cancellation_error = canceller.cancel().err();
            match result_receiver.recv_timeout(FILE_IO_CANCELLATION_GRACE) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = worker.join();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => drop(worker),
            }
            Err(file_io_timeout(operation, cancellation_error.as_ref()))
        }
    }
}

pub(super) fn cancellable_file_io_pass_is_active() -> bool {
    CANCELLABLE_FILE_IO_PASS_ACTIVE.get()
}

struct CancellableFileIoPassGuard;

impl CancellableFileIoPassGuard {
    fn enter() -> Self {
        let was_active = CANCELLABLE_FILE_IO_PASS_ACTIVE.replace(true);
        debug_assert!(!was_active, "cancellable file-I/O passes must not nest");
        Self
    }
}

impl Drop for CancellableFileIoPassGuard {
    fn drop(&mut self) {
        CANCELLABLE_FILE_IO_PASS_ACTIVE.set(false);
    }
}

fn require_file_io_before_deadline(deadline: Instant, operation: &str) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(file_io_timeout(operation, None))
    } else {
        Ok(())
    }
}

fn file_io_timeout(operation: &str, cancellation_error: Option<&io::Error>) -> io::Error {
    let suffix = cancellation_error.map_or_else(String::new, |error| {
        format!("; synchronous I/O cancellation failed: {error}")
    });
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!("{operation} exceeded its operation deadline{suffix}"),
    )
}

fn reject_alternate_data_streams(path: &Path) -> io::Result<()> {
    for component in path.components() {
        if let std::path::Component::Normal(value) = component
            && value.encode_wide().any(|unit| unit == u16::from(b':'))
        {
            return Err(invalid_data(
                path,
                "alternate data stream paths are not accepted",
            ));
        }
    }
    Ok(())
}

fn scoped_absolute_path(path: &Path) -> io::Result<PathBuf> {
    validate_windows_path_units(path)?;
    let absolute = std::path::absolute(path)?;
    validate_windows_path_units(&absolute)?;
    reject_alternate_data_streams(&absolute)?;
    let _ = wide_path(&absolute)?;
    if absolute.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(invalid_data(
            path,
            "root-scoped Windows paths must not contain dot components",
        ));
    }
    Ok(absolute)
}

fn reject_reparse_ancestors(path: &Path) -> io::Result<()> {
    let mut ancestors = path.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        let handle = open_existing_path(
            ancestor,
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        )?;
        let identity = handle_identity(handle.as_raw_handle().cast())?;
        reject_reparse_identity(ancestor, identity)?;
    }
    Ok(())
}

fn reject_reparse_identity(path: &Path, identity: WindowsFileIdentity) -> io::Result<()> {
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("refusing Windows reparse point `{}`", path.display()),
        ));
    }
    Ok(())
}

fn file_handle(file: &File) -> HANDLE {
    file.as_raw_handle().cast()
}

fn file_from_handle(handle: OwnedHandle) -> File {
    handle.into()
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    use std::path::{Component, Prefix};

    validate_windows_path_units(path)?;
    let absolute = std::path::absolute(path)?;
    validate_windows_path_units(&absolute)?;
    reject_alternate_data_streams(&absolute)?;
    if !absolute.is_absolute() {
        return Err(invalid_data(
            path,
            "raw Windows filesystem operations require an absolute path",
        ));
    }
    let prefix = match absolute.components().next() {
        Some(Component::Prefix(prefix)) => prefix.kind(),
        _ => {
            return Err(invalid_data(
                path,
                "raw Windows filesystem operations require a local drive path",
            ));
        }
    };
    let mut units = match prefix {
        Prefix::Disk(_) => vec![
            u16::from(b'\\'),
            u16::from(b'\\'),
            u16::from(b'?'),
            u16::from(b'\\'),
        ],
        Prefix::VerbatimDisk(_) => Vec::new(),
        Prefix::UNC(_, _)
        | Prefix::VerbatimUNC(_, _)
        | Prefix::DeviceNS(_)
        | Prefix::Verbatim(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "`{}` is not an accepted local Windows drive path",
                    path.display()
                ),
            ));
        }
    };
    for unit in absolute.as_os_str().encode_wide() {
        if unit == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows paths may not contain NUL characters",
            ));
        }
        if units.len().saturating_add(2) > MAX_WINDOWS_PATH_UNITS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows path exceeds the extended-length path limit",
            ));
        }
        units.push(if unit == u16::from(b'/') {
            u16::from(b'\\')
        } else {
            unit
        });
    }
    units.push(0);
    Ok(units)
}

pub(super) fn validate_windows_path_units(path: &Path) -> io::Result<()> {
    if path
        .as_os_str()
        .encode_wide()
        .take(MAX_WINDOWS_PATH_UNITS)
        .count()
        >= MAX_WINDOWS_PATH_UNITS
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows path exceeds the extended-length path limit",
        ));
    }
    Ok(())
}

fn truncate_at_nul(units: &mut Vec<u16>) {
    if let Some(nul) = units.iter().position(|unit| *unit == 0) {
        units.truncate(nul);
    }
}

fn invalid_data(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unsafe Windows path `{}`: {reason}", path.display()),
    )
}

fn last_error() -> io::Error {
    io::Error::last_os_error()
}

#[allow(
    unsafe_code,
    reason = "CreateFileW returns a fresh owned handle and receives a live NUL-terminated path"
)]
fn open_existing_path(path: &Path, access: u32, share: u32, flags: u32) -> io::Result<OwnedHandle> {
    let path = wide_path(path)?;
    // SAFETY: `path` is NUL-terminated for the duration of the call. Null
    // security/template pointers are permitted, and success transfers one
    // newly-owned handle to this function.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            share,
            std::ptr::null(),
            OPEN_EXISTING,
            flags | FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    // SAFETY: a successful CreateFileW call returned a unique owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.cast()) })
}

#[allow(
    unsafe_code,
    reason = "GetFileInformationByHandleEx writes fixed-size POD outputs for a live handle"
)]
fn query_handle_information<T: Default>(handle: HANDLE, class: i32) -> io::Result<T> {
    let mut value = T::default();
    let length = u32::try_from(std::mem::size_of::<T>()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows handle query is too large",
        )
    })?;
    // SAFETY: `value` is writable for exactly `length` bytes and its type
    // matches the information class selected by each caller.
    if unsafe { GetFileInformationByHandleEx(handle, class, (&raw mut value).cast(), length) } == 0
    {
        return Err(last_error());
    }
    Ok(value)
}

fn handle_identity(handle: HANDLE) -> io::Result<WindowsFileIdentity> {
    // SAFETY: each class is paired with its documented output structure.
    let id: FILE_ID_INFO = query_handle_information(handle, FileIdInfo)?;
    let basic: FILE_BASIC_INFO = query_handle_information(handle, FileBasicInfo)?;
    let standard: FILE_STANDARD_INFO = query_handle_information(handle, FileStandardInfo)?;
    let tag: FILE_ATTRIBUTE_TAG_INFO = query_handle_information(handle, FileAttributeTagInfo)?;
    let size = u64::try_from(standard.EndOfFile).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows reported a negative file size",
        )
    })?;
    Ok(WindowsFileIdentity {
        volume_serial_number: id.VolumeSerialNumber,
        file_id: id.FileId.Identifier,
        size,
        creation_time: basic.CreationTime,
        last_write_time: basic.LastWriteTime,
        change_time: basic.ChangeTime,
        attributes: tag.FileAttributes,
        directory: standard.Directory,
    })
}

impl NormalizedFinalPath {
    fn parse(path: &[u16]) -> io::Result<Self> {
        let (is_unc, remainder) = if let Some(remainder) = strip_ascii_prefix(path, r"\\?\UNC\") {
            (true, remainder)
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
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned a final path without a volume",
            ));
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
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned a final path with dot components",
            ));
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

#[allow(
    unsafe_code,
    reason = "GetFinalPathNameByHandleW writes into a bounded live UTF-16 buffer"
)]
fn final_path(handle: HANDLE) -> io::Result<NormalizedFinalPath> {
    for flags in [0, VOLUME_NAME_GUID] {
        let mut buffer = vec![0_u16; FINAL_PATH_BUFFER_UNITS];
        // SAFETY: `buffer` is writable for its declared length and `handle`
        // remains live for the call.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                buffer.as_mut_ptr(),
                u32::try_from(buffer.len()).unwrap_or(u32::MAX),
                flags,
            )
        };
        let length = usize::try_from(length).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows returned an oversized final path",
            )
        })?;
        if length != 0 && length < buffer.len() {
            return NormalizedFinalPath::parse(&buffer[..length]);
        }
    }
    Err(last_error())
}

#[allow(
    unsafe_code,
    reason = "volume APIs receive bounded writable UTF-16 buffers"
)]
fn get_volume_path(path: &[u16], output: &mut [u16]) -> io::Result<()> {
    let length = u32::try_from(output.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows volume buffer is too large",
        )
    })?;
    // SAFETY: both slices remain live, `path` is NUL-terminated, and `output`
    // is writable for the advertised number of UTF-16 units.
    if unsafe { GetVolumePathNameW(path.as_ptr(), output.as_mut_ptr(), length) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "GetDriveTypeW only reads a live NUL-terminated path"
)]
fn get_drive_type(volume_path: &[u16]) -> u32 {
    // SAFETY: `volume_path` is NUL-terminated for the duration of the call.
    unsafe { GetDriveTypeW(volume_path.as_ptr()) }
}

#[allow(
    unsafe_code,
    reason = "GetVolumeInformationW receives a live path and bounded filesystem-name buffer"
)]
fn get_volume_information(volume_path: &[u16], filesystem: &mut [u16]) -> io::Result<()> {
    let filesystem_length = u32::try_from(filesystem.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows filesystem buffer is too large",
        )
    })?;
    // SAFETY: the input is NUL-terminated, optional outputs are null, and the
    // filesystem-name buffer is writable for its advertised length.
    if unsafe {
        GetVolumeInformationW(
            volume_path.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            filesystem.as_mut_ptr(),
            filesystem_length,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "Job Object APIs return and consume process-lifetime kernel handles"
)]
fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    // SAFETY: null attributes/name request an unnamed, non-inheritable job.
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err(last_error());
    }
    // SAFETY: CreateJobObjectW returned a fresh owned handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let length = u32::try_from(std::mem::size_of_val(&limits)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "job limit structure is too large",
        )
    })?;
    // SAFETY: `limits` matches JobObjectExtendedLimitInformation and remains
    // live for the full call.
    if unsafe {
        SetInformationJobObject(
            handle.as_raw_handle().cast(),
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            length,
        )
    } == 0
    {
        return Err(last_error());
    }
    Ok(handle)
}

#[allow(
    unsafe_code,
    reason = "AssignProcessToJobObject receives live job and Child process handles"
)]
fn assign_process_to_job(job: HANDLE, process: HANDLE) -> io::Result<()> {
    // SAFETY: both handles remain owned by their callers for the duration of
    // the assignment call.
    if unsafe { AssignProcessToJobObject(job, process) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "TerminateJobObject receives a live owned job handle"
)]
fn terminate_job(job: HANDLE) -> io::Result<()> {
    // SAFETY: `job` remains owned and live for the duration of the call.
    if unsafe { TerminateJobObject(job, 1) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "Toolhelp enumeration owns its snapshot and validates the sole suspended thread"
)]
fn sole_process_thread(process_id: u32, deadline: Instant) -> io::Result<OwnedHandle> {
    let mut snapshot = None;
    for attempt in 0..MAX_TOOLHELP_SNAPSHOT_RETRIES {
        require_before_deadline(deadline, "Windows thread snapshot")?;
        // SAFETY: the snapshot request has no pointer arguments and returns a
        // new owned handle on success.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if raw != INVALID_HANDLE_VALUE {
            // SAFETY: the successful call returned a unique owned handle.
            snapshot = Some(unsafe { OwnedHandle::from_raw_handle(raw.cast()) });
            break;
        }
        let error = last_error();
        if error.raw_os_error() != Some(ERROR_BAD_LENGTH as i32) {
            return Err(error);
        }
        if attempt.saturating_add(1) == MAX_TOOLHELP_SNAPSHOT_RETRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows thread snapshot remained unstable after bounded retries",
            ));
        }
    }
    let snapshot = snapshot.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows thread snapshot was not acquired",
        )
    })?;
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(std::mem::size_of::<THREADENTRY32>()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "thread entry is too large")
        })?,
        ..THREADENTRY32::default()
    };
    let mut thread_id = None;
    let mut inspected = 0_usize;
    // SAFETY: `entry` is writable and advertises its correct structure size.
    let mut present = unsafe { Thread32First(snapshot.as_raw_handle().cast(), &raw mut entry) };
    if present == 0 {
        let error = last_error();
        if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
            return Err(error);
        }
    }
    while present != 0 {
        require_before_deadline(deadline, "Windows thread enumeration")?;
        inspected = inspected.saturating_add(1);
        if inspected > MAX_TOOLHELP_THREAD_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows thread snapshot exceeds its entry limit",
            ));
        }
        if entry.th32OwnerProcessID == process_id && thread_id.replace(entry.th32ThreadID).is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a suspended child unexpectedly had more than one thread before containment",
            ));
        }
        entry.dwSize = u32::try_from(std::mem::size_of::<THREADENTRY32>()).unwrap_or(u32::MAX);
        // SAFETY: the snapshot and writable entry remain live.
        present = unsafe { Thread32Next(snapshot.as_raw_handle().cast(), &raw mut entry) };
        if present == 0 {
            let error = last_error();
            if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
                return Err(error);
            }
        }
    }
    let thread_id = thread_id.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the suspended child primary thread could not be found",
        )
    })?;
    require_before_deadline(deadline, "Windows primary thread open")?;
    // SAFETY: the ID came from the live system snapshot; the returned handle
    // is non-inheritable and uniquely owned on success.
    let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    if raw.is_null() {
        return Err(last_error());
    }
    // SAFETY: OpenThread returned a fresh owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw.cast()) })
}

fn require_before_deadline(deadline: Instant, operation: &str) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("{operation} exceeded its operation deadline"),
        ));
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "ResumeThread receives the live sole primary-thread handle"
)]
fn resume_thread(thread: HANDLE) -> io::Result<()> {
    // SAFETY: `thread` has THREAD_SUSPEND_RESUME access and remains live.
    if unsafe { ResumeThread(thread) } == u32::MAX {
        return Err(last_error());
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "DuplicateHandle turns the current-thread pseudo-handle into an owned real handle"
)]
fn duplicate_current_thread_handle() -> io::Result<OwnedHandle> {
    let mut duplicate: HANDLE = std::ptr::null_mut();
    // SAFETY: the source pseudo-handle and process pseudo-handles are always
    // valid in this process; `duplicate` is a writable handle output.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            GetCurrentThread(),
            GetCurrentProcess(),
            &raw mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(last_error());
    }
    // SAFETY: DuplicateHandle returned a fresh owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(duplicate.cast()) })
}

#[allow(
    unsafe_code,
    reason = "CancelSynchronousIo receives an owned real handle for the target reader thread"
)]
fn cancel_synchronous_io(thread: HANDLE) -> io::Result<()> {
    // SAFETY: `thread` remains live and was duplicated with its original access.
    if unsafe { CancelSynchronousIo(thread) } == 0 {
        let error = last_error();
        if error.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::windows::fs::symlink_dir;
    use std::process::Command;

    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_APPEND_DATA, FILE_DELETE_CHILD,
        FILE_WRITE_DATA,
    };

    use super::*;

    #[test]
    fn go_command_paths_use_unambiguous_dos_spelling() {
        for path in [
            r"\\?\C:\repo\app",
            r"C:\repo\app",
            r"C:/repo/app",
            r"C:\repo\a b\café\a.b",
        ] {
            let converted = go_command_path(Path::new(path)).expect("safe Go command path");
            assert!(!converted.to_string_lossy().starts_with(r"\\?\"));
            assert!(!converted.to_string_lossy().contains('/'));
        }
        assert_eq!(
            go_command_path(Path::new(r"\\?\C:\repo\app")).expect("verbatim drive path"),
            Path::new(r"C:\repo\app")
        );
    }

    #[test]
    fn go_command_paths_reject_win32_namespace_ambiguity() {
        for path in [
            r"repo\app",
            r"\\server\share\app",
            r"\\?\UNC\server\share\app",
            r"\\.\C:\repo\app",
            r"C:\repo\trailing.",
            "C:\\repo\\trailing ",
            r"C:\repo\NUL.txt",
            r"C:\repo\conin$.log",
            r"C:\repo\COM1",
            r"C:\repo\lpt².log",
            r"C:\repo\bad<name",
            "C:\\repo\\control\u{0001}name",
        ] {
            assert!(
                go_command_path(Path::new(path)).is_err(),
                "unsafe path was accepted: {path:?}"
            );
        }
    }

    #[test]
    fn go_command_paths_reject_lossy_unicode_and_overlong_working_directories() {
        fn path_with_units(units: usize, trailing_separator: bool) -> PathBuf {
            let prefix = r"C:\segment\";
            let separator_units = usize::from(trailing_separator);
            let suffix_units = units
                .checked_sub(prefix.encode_utf16().count() + separator_units)
                .expect("requested path length covers its prefix");
            let mut path = format!("{prefix}{}", "a".repeat(suffix_units));
            if trailing_separator {
                path.push('\\');
            }
            assert_eq!(path.encode_utf16().count(), units);
            PathBuf::from(path)
        }

        let mut non_unicode = r"C:\repo\".encode_utf16().collect::<Vec<_>>();
        non_unicode.push(0xD800);
        let non_unicode = PathBuf::from(OsString::from_wide(&non_unicode));
        assert!(go_command_path(&non_unicode).is_err());

        for (units, trailing_separator, accepted) in [
            (258, false, true),
            (259, false, false),
            (259, true, true),
            (260, true, false),
        ] {
            let path = path_with_units(units, trailing_separator);
            assert!(go_command_path(&path).is_ok());
            assert_eq!(
                go_command_working_directory(&path).is_ok(),
                accepted,
                "unexpected {units}-unit cwd result with trailing separator={trailing_separator}"
            );
        }
    }

    fn create_directory_reparse(link: &Path, target: &Path) {
        const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

        match symlink_dir(target, link) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) => {
                let output = Command::new("cmd")
                    .args(["/D", "/C", "mklink", "/J"])
                    .arg(link)
                    .arg(target)
                    .output()
                    .expect("run junction fallback");
                assert!(
                    output.status.success(),
                    "junction fallback failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => panic!("create directory reparse point: {error}"),
        }
    }

    #[allow(
        unsafe_code,
        reason = "the regression temporarily installs a bounded empty ACL through one recovery handle"
    )]
    fn install_empty_dacl(path: &Path) -> OwnedHandle {
        let handle = open_existing_path(
            path,
            FILE_READ_ATTRIBUTES | READ_CONTROL | WRITE_DAC | WRITE_OWNER,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_OPEN_REPARSE_POINT,
        )
        .expect("open access-recovery handle");
        let acl_bytes = std::mem::size_of::<ACL>();
        let acl_words = acl_bytes.div_ceil(std::mem::size_of::<u32>());
        let mut acl = vec![0_u32; acl_words];
        let acl_length = u32::try_from(acl.len() * std::mem::size_of::<u32>())
            .expect("empty ACL size fits in u32");
        let acl_ptr = acl.as_mut_ptr().cast::<ACL>();
        // SAFETY: `acl` is aligned and writable for its advertised size.
        assert_ne!(
            unsafe { InitializeAcl(acl_ptr, acl_length, ACL_REVISION) },
            0
        );
        // SAFETY: the recovery handle and initialized ACL remain live for the
        // call; only the protected DACL is replaced.
        let status = unsafe {
            SetSecurityInfo(
                handle.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl_ptr,
                std::ptr::null(),
            )
        };
        assert_eq!(status, ERROR_SUCCESS);
        handle
    }

    #[test]
    fn sealed_acl_mask_omits_file_and_directory_mutation_rights() {
        let sealed = private_access_mask(true);
        let forbidden_file = FILE_WRITE_DATA | FILE_APPEND_DATA | DELETE;
        let forbidden_directory =
            FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY | FILE_DELETE_CHILD | DELETE;

        assert_eq!(sealed, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE);
        assert_eq!(sealed & forbidden_file, 0);
        assert_eq!(sealed & forbidden_directory, 0);
        assert_eq!(private_access_mask(false), FILE_ALL_ACCESS);
    }

    #[test]
    fn locality_gate_accepts_fixed_storage_and_rejects_alternate_streams() {
        let temp = tempfile::tempdir().expect("temporary directory");

        require_local_fixed_volume(temp.path()).expect("temporary directory is local storage");

        let alternate_stream = temp.path().join("payload:stream");
        let error = require_local_fixed_volume(&alternate_stream)
            .expect_err("alternate data stream paths must be rejected before access");
        assert!(error.to_string().contains("alternate data stream"));
    }

    #[test]
    fn locality_gate_rejects_unc_paths_before_ancestor_inspection() {
        let unc = Path::new(r"\\polint-invalid-host\share\candidate");

        let error = require_local_creation_volume(unc)
            .expect_err("UNC creation paths must be rejected as non-local");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .contains("accepted local Windows drive path")
        );
    }

    #[test]
    fn containing_volume_gate_ignores_but_recursive_gate_rejects_descendant_reparse_points() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("root");
        let target = temp.path().join("target");
        fs::create_dir(&root).expect("create certification root");
        fs::create_dir(&target).expect("create reparse target");
        let link = root.join("descendant-link");
        create_directory_reparse(&link, &target);

        require_local_fixed_volume(&root)
            .expect("containing-only certification ignores unrelated descendants");
        let error = require_local_tree(&root)
            .expect_err("recursive certification must reject descendant reparse points");
        assert!(error.to_string().contains("reparse point"));
    }

    #[test]
    fn recursive_local_tree_gate_preserves_deadline_typing() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let error = require_local_tree_with_limits_until(
            temp.path(),
            Instant::now(),
            LocalTreeCertificationLimits::DEFAULT,
        )
        .expect_err("an expired deadline must stop certification");

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn recursive_local_tree_gate_rejects_inclusions_below_exclusions() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let excluded = temp.path().join("cache");
        let selected = excluded.join("selected");
        fs::create_dir_all(&selected).expect("create selected excluded directory");

        let error = require_local_tree_with_exclusions_and_limits_until(
            temp.path(),
            std::slice::from_ref(&excluded),
            std::slice::from_ref(&selected),
            true,
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits::DEFAULT,
        )
        .expect_err("an included tree below an exclusion must fail closed");

        assert!(error.to_string().contains("must not overlap"));
    }

    #[test]
    fn scoped_path_index_keeps_component_prefix_queries_logarithmic_and_exact() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = scoped_absolute_path(temp.path()).expect("absolute temporary root");
        let punctuation_sibling = root.join("a-b");
        let nested = root.join("a/deep");
        let paths = vec![punctuation_sibling.clone(), nested.clone()];
        let index = ScopedPathIndex::new(
            &paths,
            Instant::now() + Duration::from_secs(5),
            "scoped-path index regression",
        )
        .expect("build scoped-path index");

        assert!(index.contains_descendant_of(&root.join("a")));
        assert!(index.contains_exact(&punctuation_sibling));
        assert!(!index.contains_exact(&root.join("a")));
        assert!(index.contains_ancestor_of(&nested.join("child"), &root));
        assert!(!index.contains_ancestor_of(&root.join("unrelated"), &root));
    }

    #[test]
    fn recursive_local_tree_gate_bounds_entries_depth_and_frontier() {
        let entry_root = tempfile::tempdir().expect("entry-limit directory");
        fs::write(entry_root.path().join("one"), b"one").expect("write first entry");
        fs::write(entry_root.path().join("two"), b"two").expect("write second entry");
        let entry_error = require_local_tree_with_limits_until(
            entry_root.path(),
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits {
                entries: 1,
                depth: 8,
                frontier: 8,
                frontier_path_units: 1_024,
            },
        )
        .expect_err("entry limit must fail closed");
        assert!(entry_error.to_string().contains("entry count"));

        let filtered_root = tempfile::tempdir().expect("filtered entry-limit directory");
        fs::write(filtered_root.path().join(".one"), b"one").expect("write first filtered entry");
        fs::write(filtered_root.path().join("_two"), b"two").expect("write second filtered entry");
        let filtered_error = require_local_tree_with_exclusions_and_limits_until(
            filtered_root.path(),
            &[],
            &[],
            true,
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits {
                entries: 1,
                depth: 8,
                frontier: 8,
                frontier_path_units: 1_024,
            },
        )
        .expect_err("filtered entries must still consume the independent entry budget");
        assert!(filtered_error.to_string().contains("entry count"));

        let depth_root = tempfile::tempdir().expect("depth-limit directory");
        fs::create_dir_all(depth_root.path().join("one/two")).expect("create nested directories");
        let depth_error = require_local_tree_with_limits_until(
            depth_root.path(),
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits {
                entries: 8,
                depth: 1,
                frontier: 8,
                frontier_path_units: 1_024,
            },
        )
        .expect_err("depth limit must fail closed");
        assert!(depth_error.to_string().contains("directory depth"));

        let frontier_root = tempfile::tempdir().expect("frontier-limit directory");
        fs::create_dir(frontier_root.path().join("one")).expect("create first child directory");
        fs::create_dir(frontier_root.path().join("two")).expect("create second child directory");
        let frontier_error = require_local_tree_with_limits_until(
            frontier_root.path(),
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits {
                entries: 8,
                depth: 8,
                frontier: 1,
                frontier_path_units: 1_024,
            },
        )
        .expect_err("frontier limit must fail closed");
        assert!(
            frontier_error
                .to_string()
                .contains("pending-directory frontier")
        );

        let path_budget_root = tempfile::tempdir().expect("path-budget directory");
        fs::create_dir(path_budget_root.path().join("child")).expect("create path-budget child");
        let root_units = scoped_absolute_path(path_budget_root.path())
            .expect("absolute path-budget root")
            .as_os_str()
            .encode_wide()
            .count();
        let path_budget_error = require_local_tree_with_limits_until(
            path_budget_root.path(),
            Instant::now() + Duration::from_secs(5),
            LocalTreeCertificationLimits {
                entries: 8,
                depth: 8,
                frontier: 8,
                frontier_path_units: root_units.saturating_add(1),
            },
        )
        .expect_err("frontier path-unit limit must fail closed");
        assert!(
            path_budget_error
                .to_string()
                .contains("pending-directory path units")
        );
    }

    #[test]
    fn raw_paths_use_extended_local_drive_form_and_reject_unc() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let wide = wide_path(temp.path()).expect("encode local path");
        assert!(wide.starts_with(&[
            u16::from(b'\\'),
            u16::from(b'\\'),
            u16::from(b'?'),
            u16::from(b'\\'),
        ]));

        let unc = Path::new(r"\\server\share\semantic-cache");
        let error = wide_path(unc).expect_err("UNC paths must remain rejected");
        assert!(error.to_string().contains("local Windows drive path"));
    }

    #[test]
    fn oversized_paths_are_rejected_before_absolute_path_allocation() {
        use std::os::windows::ffi::OsStringExt;

        let units = vec![u16::from(b'x'); MAX_WINDOWS_PATH_UNITS];
        let path = PathBuf::from(std::ffi::OsString::from_wide(&units));

        let error = wide_path(&path)
            .expect_err("an oversized path must fail before it is absolutized or collected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("extended-length path limit"));
    }

    #[test]
    fn private_file_operations_support_paths_beyond_max_path() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let mut directory = temp.path().join("private-long-path");
        create_private_directory(&directory).expect("create private root");
        let mut index = 0_u32;
        while directory.as_os_str().encode_wide().count() <= 300 {
            directory.push(format!("segment-{index:04}-abcdefghijklmnop"));
            create_private_directory(&directory).expect("create long private directory");
            index = index.checked_add(1).expect("bounded segment count");
        }
        assert!(directory.as_os_str().encode_wide().count() > 260);

        let file = directory.join("payload");
        create_private_file(&file, b"extended-path", false).expect("create file beyond MAX_PATH");
        let contents = SecureFile::open_regular_no_follow(&file)
            .and_then(|file| file.read_bounded_until(1024, Instant::now() + Duration::from_secs(5)))
            .expect("read file beyond MAX_PATH");
        assert_eq!(contents.bytes, b"extended-path");
    }

    #[test]
    fn cancellable_file_io_returns_after_a_blocking_worker_ignores_cancellation() {
        let started = Instant::now();
        let error = run_cancellable_file_io(
            started + Duration::from_millis(10),
            "blocking file-I/O probe",
            || {
                thread::sleep(Duration::from_millis(300));
                Ok(())
            },
        )
        .expect_err("the absolute deadline must win");

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_millis(250),
            "deadline cleanup must not join an unresponsive I/O worker"
        );
    }

    #[test]
    fn certification_pass_amortizes_many_secure_reads_over_one_worker() {
        FILE_IO_WORKER_SPAWNS.set(0);
        let temp = tempfile::tempdir().expect("temporary directory");
        let mut paths = Vec::new();
        for ordinal in 0..64 {
            let path = temp.path().join(format!("input-{ordinal:03}"));
            fs::write(&path, b"dependency input").expect("write dependency input");
            paths.push(path);
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        let root = temp.path().to_path_buf();
        let read_count = run_cancellable_file_io_pass(
            deadline,
            "multi-file certification probe",
            move || -> io::Result<usize> {
                let scope = certified_local_tree_until(&root, &[], deadline)?;
                if scope.root() != root.as_path() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "certified scope returned a different root",
                    ));
                }
                for path in &paths {
                    let contents = scope
                        .open_regular_no_follow(path)?
                        .read_bounded_until(1024, deadline)?;
                    if contents.bytes != b"dependency input" {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "dependency input contents changed",
                        ));
                    }
                }
                Ok(paths.len())
            },
        )
        .expect("start cancellable certification worker")
        .expect("read all dependency inputs");

        assert_eq!(read_count, 64);
        assert_eq!(FILE_IO_WORKER_SPAWNS.get(), 1);
    }

    #[test]
    fn root_scoped_identity_rejects_reparse_escape_but_names_the_direct_poison() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir(&root).expect("create certified root");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("payload"), b"outside").expect("write outside payload");
        let link = root.join("link");
        let escaped_payload = link.join("payload");
        let deadline = Instant::now() + Duration::from_secs(10);

        let (escape_error, poison_identity) = run_cancellable_file_io_pass(
            deadline,
            "root-scoped reparse regression",
            move || -> io::Result<(io::Error, WindowsFileIdentity)> {
                let scope = certified_local_tree_until(&root, &[], deadline)?;
                create_directory_reparse(&link, &outside);
                let escape_error = scope
                    .identity_no_follow(&escaped_payload)
                    .expect_err("an intermediate reparse must not escape the certified root");
                let poison_identity = scope.direct_child_identity_allow_reparse(&link)?;
                Ok((escape_error, poison_identity))
            },
        )
        .expect("start root-scoped regression worker")
        .expect("capture root-scoped reparse behavior");

        assert!(escape_error.to_string().contains("reparse boundary"));
        assert_ne!(poison_identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    }

    #[test]
    fn root_scoped_identity_rejects_a_different_volume_number() {
        let root = WindowsFileIdentity {
            volume_serial_number: 7,
            file_id: [1; 16],
            size: 0,
            creation_time: 0,
            last_write_time: 0,
            change_time: 0,
            attributes: FILE_ATTRIBUTE_NORMAL,
            directory: true,
        };
        let mut candidate = root;
        candidate.volume_serial_number = 8;

        let error = require_same_scoped_volume(Path::new(r"C:\root\entry"), root, candidate)
            .expect_err("a different volume must fail closed");

        assert!(error.to_string().contains("different Windows volume"));
    }

    #[test]
    fn root_scoped_read_execute_projection_rejects_access_denial() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let root = temp.path().join("root");
        fs::create_dir(&root).expect("create certified root");
        let file = root.join("tool");
        create_private_file(&file, b"tool", false).expect("create owner-only tool file");
        let deadline = Instant::now() + Duration::from_secs(10);

        let (projection, private_projection, denied) = run_cancellable_file_io_pass(
            deadline,
            "root-scoped access regression",
            move || -> io::Result<(u32, u32, io::Result<WindowsScopedFileState>)> {
                let scope = certified_local_tree_until(&root, &[], deadline)?;
                let projection = scope
                    .read_execute_state(&file, false)?
                    .effective_access
                    .projection();
                let private_projection = scope.private_state(&file, false)?.1.projection();
                let recovery = install_empty_dacl(&file);
                let denied = scope.read_execute_state(&file, false);
                set_handle_private_dacl(recovery.as_raw_handle().cast(), false, false)
                    .expect("restore owner-only access through the recovery handle");
                Ok((projection, private_projection, denied))
            },
        )
        .expect("start root-scoped access worker")
        .expect("capture access projection");

        assert_eq!(
            projection,
            WindowsEffectiveAccess::READ_EXECUTE.projection()
        );
        assert_eq!(
            private_projection,
            WindowsPrivateAccess::Mutable.projection()
        );
        assert_eq!(
            denied
                .expect_err("an empty DACL must deny normalized read/execute access")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn binding_identity_ignores_mutable_metadata_but_not_object_identity() {
        let baseline = WindowsFileIdentity {
            volume_serial_number: 7,
            file_id: [3; 16],
            size: 1,
            creation_time: 2,
            last_write_time: 3,
            change_time: 4,
            attributes: FILE_ATTRIBUTE_NORMAL,
            directory: true,
        };
        let mut populated = baseline;
        populated.size = 4096;
        populated.creation_time = 20;
        populated.last_write_time = 30;
        populated.change_time = 40;
        populated.attributes |= FILE_ATTRIBUTE_READONLY;
        assert!(baseline.names_same_object(populated));

        let mut replacement = populated;
        replacement.file_id[0] ^= 1;
        assert!(!baseline.names_same_object(replacement));
        replacement = populated;
        replacement.attributes |= FILE_ATTRIBUTE_REPARSE_POINT;
        assert!(!baseline.names_same_object(replacement));
    }

    #[test]
    fn locality_gate_rejects_reparse_ancestors() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let target = temp.path().join("target");
        fs::create_dir(&target).expect("create symlink target");
        fs::write(target.join("payload"), b"local").expect("create target payload");
        let link = temp.path().join("link");
        create_directory_reparse(&link, &target);

        assert!(path_is_reparse_point(&link).expect("inspect directory symlink"));
        let error = require_local_fixed_volume(&link.join("payload"))
            .expect_err("a reparse ancestor must not be followed");
        assert!(error.to_string().contains("reparse point"));
    }

    #[test]
    fn file_url_is_a_local_encoded_dos_url() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let proxy = temp.path().join("module with space");
        fs::create_dir(&proxy).expect("create proxy directory");

        let url = file_url(&proxy).expect("create local file URL");

        assert!(url.starts_with("file:///"), "unexpected file URL: {url}");
        assert!(url.contains("module%20with%20space"));
        assert!(!url.contains('\\'));
        assert!(!url.contains(r"\\?\"));
    }

    #[test]
    fn private_acl_transitions_are_enforced_by_windows() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let private = temp.path().join("private");
        create_private_directory(&private).expect("create protected private directory");
        verify_private_path(&private, true, false).expect("verify mutable directory DACL");
        let guard = PinnedDirectoryGuard::open(&private).expect("pin protected directory");

        let file = private.join("payload");
        create_private_file(&file, b"mutable", false).expect("create mutable private file");
        guard
            .verify_path_binding()
            .expect("creating a child must preserve the directory object binding");
        verify_private_path(&file, false, false).expect("verify mutable file DACL");
        overwrite_private_file(&file, b"updated").expect("rewrite mutable private file");

        seal_private_path(&file, false).expect("seal private file");
        seal_private_path(&file, false).expect("sealing a private file must be idempotent");
        verify_private_path(&file, false, true).expect("verify sealed file DACL");
        assert!(
            fs::OpenOptions::new().write(true).open(&file).is_err(),
            "the sealed DACL must deny write-data access"
        );

        seal_private_path(&private, true).expect("seal private directory");
        verify_private_path(&private, true, true).expect("verify sealed directory DACL");
        assert!(
            fs::write(private.join("forbidden"), b"no").is_err(),
            "the sealed directory DACL must deny child creation"
        );

        make_private_path_writable(&private, true).expect("reopen private directory");
        make_private_path_writable(&file, false).expect("reopen private file");
        overwrite_private_file(&file, b"cleanup").expect("rewrite reopened private file");
        drop(guard);
        fs::remove_file(&file).expect("remove reopened file");
        fs::remove_dir(&private).expect("remove reopened directory");
    }

    #[test]
    fn ordinary_directory_owner_is_normalized_before_private_use() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let ordinary = temp.path().join("ordinary");
        fs::create_dir(&ordinary).expect("create ordinary directory");

        make_private_path_writable(&ordinary, true).expect("normalize inherited owner and access");
        verify_private_path(&ordinary, true, false)
            .expect("ordinary directory becomes current-user private");
    }

    #[test]
    fn ordinary_file_owner_is_normalized_when_sealed() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let ordinary = temp.path().join("ordinary");
        fs::write(&ordinary, b"payload").expect("create ordinary file");

        seal_private_path(&ordinary, false).expect("normalize and seal ordinary file");
        verify_private_path(&ordinary, false, true)
            .expect("ordinary file becomes current-user private and sealed");
        make_private_path_writable(&ordinary, false).expect("reopen normalized file");
        verify_private_path(&ordinary, false, false)
            .expect("normalized file becomes writable private");
    }

    #[test]
    fn expired_deadline_prevents_windows_thread_snapshot() {
        let error = sole_process_thread(std::process::id(), Instant::now())
            .expect_err("an expired deadline must prevent system-wide enumeration");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
