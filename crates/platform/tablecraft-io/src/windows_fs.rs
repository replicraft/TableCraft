//! Checked Windows filesystem calls shared by required publication consumers.
//! Namespace durability policy and resource accounting remain with each caller.
use crate::IoError;
use std::fs::File;
use std::io::ErrorKind;
use std::mem::{offset_of, size_of, size_of_val};
#[cfg(feature = "filesystem-journal")]
use std::os::windows::io::IntoRawHandle;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr::{null, null_mut};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_NON_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
    FILE_WRITE_THROUGH, FileFsDeviceInformation, NtCreateFile, NtQueryVolumeInformationFile,
};
use windows_sys::Wdk::System::SystemServices::FILE_FS_DEVICE_INFORMATION;
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, RtlNtStatusToDosError,
    UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, DELETE, FILE_ADD_FILE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_DISPOSITION_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO,
    FILE_INFO_BY_HANDLE_CLASS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FileAttributeTagInfo, FileDispositionInfo,
    FileIdInfo, FileRenameInfo, GetFileInformationByHandleEx, GetVolumeInformationByHandleW,
    OPEN_EXISTING, SYNCHRONIZE, SetFileInformationByHandle,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

#[cfg(feature = "filesystem-journal")]
use windows_sys::{
    Wdk::Storage::FileSystem::FILE_OPEN,
    Win32::{
        Foundation::{
            CloseHandle, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION, OBJ_CASE_INSENSITIVE,
        },
        Storage::FileSystem::{FILE_STANDARD_INFO, FileStandardInfo},
    },
};

mod location;
pub(crate) use location::Location;

#[cfg(feature = "filesystem-journal")]
mod security;

#[cfg(feature = "filesystem-journal")]
pub(crate) const PRIVATE_SECURITY_BYTES: u64 = security::WORK_BYTES;
#[cfg(feature = "filesystem-journal")]
pub(crate) const PRIVATE_SECURITY_STEPS: u64 = security::WORK_STEPS;

pub(crate) const MAX_COMPONENT_UNITS: usize = 255;
const FILE_REMOTE_DEVICE: u32 = 0x10;
const FILE_DEVICE_DISK: u32 = 7;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    pub(crate) volume: u64,
    pub(crate) file_id: [u8; 16],
}

pub(crate) struct CreateFailure {
    pub(crate) cause: IoError,
    pub(crate) entry_may_remain: bool,
}

/// A private sidecar qualification failure retains any actual created handle.
/// No file is fabricated when creation itself did not yield a valid handle.
#[cfg(feature = "filesystem-journal")]
pub(crate) struct PrivateCreateFailure {
    pub(crate) cause: IoError,
    pub(crate) entry_may_remain: bool,
    pub(crate) file: Option<File>,
}

#[cfg(feature = "filesystem-journal")]
impl From<CreateFailure> for PrivateCreateFailure {
    fn from(failure: CreateFailure) -> Self {
        Self {
            cause: failure.cause,
            entry_may_remain: failure.entry_may_remain,
            file: None,
        }
    }
}

// Each supported output owns its matching information class. Callers cannot
// combine an arbitrary output layout with a different Windows information class.
trait HandleInformation: Default {
    const CLASS: FILE_INFO_BY_HANDLE_CLASS;
}

impl HandleInformation for FILE_ID_INFO {
    const CLASS: FILE_INFO_BY_HANDLE_CLASS = FileIdInfo;
}

impl HandleInformation for FILE_ATTRIBUTE_TAG_INFO {
    const CLASS: FILE_INFO_BY_HANDLE_CLASS = FileAttributeTagInfo;
}

#[cfg(feature = "filesystem-journal")]
impl HandleInformation for FILE_STANDARD_INFO {
    const CLASS: FILE_INFO_BY_HANDLE_CLASS = FileStandardInfo;
}

fn last_error() -> IoError {
    std::io::Error::last_os_error().into()
}

fn invalid() -> IoError {
    IoError::Io(ErrorKind::InvalidInput)
}

fn handle(file: &File) -> HANDLE {
    file.as_raw_handle()
}

fn query<T: HandleInformation>(file: &File) -> Result<T, IoError> {
    let mut information = T::default();
    // SAFETY: information is a correctly aligned writable output of the selected
    // Windows ABI structure. The private trait binds that structure to its class.
    let success = unsafe {
        GetFileInformationByHandleEx(
            handle(file),
            T::CLASS,
            (&mut information as *mut T).cast(),
            size_of::<T>() as u32,
        )
    };
    if success == 0 {
        return Err(last_error());
    }
    Ok(information)
}

pub(crate) fn identity(file: &File) -> Result<FileIdentity, IoError> {
    let information: FILE_ID_INFO = query(file)?;
    Ok(FileIdentity {
        volume: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

pub(crate) fn flush(file: &File) -> Result<(), IoError> {
    // On Windows, Rust's sync_all invokes FlushFileBuffers on this same retained
    // handle, preserving the file and directory flush behavior and error path.
    file.sync_all().map_err(Into::into)
}

/// Open a bounded terminated absolute directory path without following its
/// final reparse point. The caller validates the complete path before this call.
/// Omitting delete sharing retains the selected parent against rename.
pub(crate) fn open_parent(path: &[u16]) -> Result<File, IoError> {
    if path.last() != Some(&0) {
        return Err(invalid());
    }
    // SAFETY: path is terminated, bounded UTF-16. No inherited handle;
    // OPEN_REPARSE_POINT exposes the final parent rather than following it.
    // Omission of FILE_SHARE_DELETE holds this directory against rename.
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | FILE_ADD_FILE | FILE_TRAVERSE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    // SAFETY: successful CreateFileW returned unique ownership.
    let parent = unsafe { File::from_raw_handle(raw) };
    let attributes: FILE_ATTRIBUTE_TAG_INFO = query(&parent)?;
    if attributes.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(IoError::Io(ErrorKind::Unsupported));
    }
    Ok(parent)
}

pub(crate) fn qualify_local_ntfs(parent: &File) -> Result<(), IoError> {
    let mut name = [0u16; 32];
    // SAFETY: optional outputs are null; name is a live writable UTF-16 buffer.
    let success = unsafe {
        GetVolumeInformationByHandleW(
            handle(parent),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if success == 0 {
        return Err(last_error());
    }
    if name[..5] != [78, 84, 70, 83, 0] {
        return Err(IoError::Io(ErrorKind::Unsupported));
    }
    let mut device = FILE_FS_DEVICE_INFORMATION::default();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: synchronous directory handle and exact FILE_FS_DEVICE_INFORMATION
    // layout; output lives until the synchronous query completes.
    let result = unsafe {
        NtQueryVolumeInformationFile(
            handle(parent),
            &mut status,
            (&mut device as *mut FILE_FS_DEVICE_INFORMATION).cast(),
            size_of::<FILE_FS_DEVICE_INFORMATION>() as u32,
            FileFsDeviceInformation,
        )
    };
    if result < 0 {
        return Err(nt_error(result));
    }
    if device.DeviceType != FILE_DEVICE_DISK || device.Characteristics & FILE_REMOTE_DEVICE != 0 {
        return Err(IoError::Io(ErrorKind::Unsupported));
    }
    Ok(())
}

fn nt_error(status: i32) -> IoError {
    // SAFETY: pure conversion of an NTSTATUS value, without borrowed memory.
    let code = unsafe { RtlNtStatusToDosError(status) };
    std::io::Error::from_raw_os_error(code as i32).into()
}

/// Create a new write-through regular file relative to a retained directory.
/// The caller supplies a prevalidated ordinary single component and owns its
/// resource charges. Success excludes other opens; an existing entry is untouched.
pub(crate) fn create_exclusive_write_through(
    parent: &File,
    name: &mut [u16],
) -> Result<File, CreateFailure> {
    relative_file(
        parent,
        name,
        null_mut(),
        FILE_CREATE,
        GENERIC_READ | GENERIC_WRITE | DELETE | SYNCHRONIZE,
    )
}

fn relative_file(
    parent: &File,
    name: &mut [u16],
    descriptor: windows_sys::Win32::Security::PSECURITY_DESCRIPTOR,
    disposition: u32,
    access: u32,
) -> Result<File, CreateFailure> {
    if name.is_empty() || name.len() > MAX_COMPONENT_UNITS {
        return Err(CreateFailure {
            cause: invalid(),
            entry_may_remain: false,
        });
    }
    let mut unicode = UNICODE_STRING {
        Length: size_of_val(name) as u16,
        MaximumLength: size_of_val(name) as u16,
        Buffer: name.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: handle(parent),
        ObjectName: &mut unicode,
        Attributes: relative_attributes(disposition),
        SecurityDescriptor: descriptor.cast(),
        ..Default::default()
    };
    let mut raw = null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: wrappers supply a validated simple name and retained parent.
    // Any non-null descriptor is live throughout this call via with_descriptor.
    // FILE_CREATE never replaces; FILE_OPEN neither creates nor truncates.
    // Share 0 denies other opens, including rename/delete handles.
    let result = unsafe {
        NtCreateFile(
            &mut raw,
            access,
            &attributes,
            &mut status,
            null(),
            FILE_ATTRIBUTE_NORMAL,
            0,
            disposition,
            FILE_NON_DIRECTORY_FILE
                | FILE_SYNCHRONOUS_IO_NONALERT
                | FILE_WRITE_THROUGH
                | FILE_OPEN_REPARSE_POINT,
            null(),
            0,
        )
    };
    if result < 0 {
        let error = relative_error(result, disposition);
        return Err(CreateFailure {
            cause: error,
            entry_may_remain: false,
        });
    }
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(CreateFailure {
            cause: IoError::Io(ErrorKind::InvalidData),
            entry_may_remain: disposition == FILE_CREATE,
        });
    }
    // SAFETY: NtCreateFile succeeded and transfers unique handle ownership.
    let file = unsafe { File::from_raw_handle(raw) };
    Ok(file)
}

/// Rename through the retained file into a retained parent without replacement.
/// The caller supplies a prevalidated ordinary component and handles uncertain
/// visibility on failure; success alone is not a terminal durability barrier.
pub(crate) fn rename_no_replace(file: &File, parent: &File, name: &[u16]) -> Result<(), IoError> {
    if name.is_empty() || name.len() > MAX_COMPONENT_UNITS {
        return Err(invalid());
    }
    // Fixed maximum buffer retains FILE_RENAME_INFO alignment and avoids
    // any allocation beyond the visibility boundary. Only initialized bytes
    // through the supplied UTF-16 name are passed to Windows.
    let mut storage = [0usize;
        (size_of::<FILE_RENAME_INFO>() + MAX_COMPONENT_UNITS * size_of::<u16>())
            .div_ceil(size_of::<usize>())];
    let header = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    let name_bytes = size_of_val(name);
    let length = offset_of!(FILE_RENAME_INFO, FileName) + name_bytes;
    // SAFETY: storage is sufficiently sized/aligned for header plus bounded
    // name. RootDirectory remains alive and ReplaceIfExists stays false.
    let success = unsafe {
        (*header).Anonymous.ReplaceIfExists = false;
        (*header).RootDirectory = handle(parent);
        (*header).FileNameLength = name_bytes as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            storage
                .as_mut_ptr()
                .cast::<u8>()
                .add(offset_of!(FILE_RENAME_INFO, FileName))
                .cast::<u16>(),
            name.len(),
        );
        SetFileInformationByHandle(handle(file), FileRenameInfo, header.cast(), length as u32)
    };
    if success == 0 {
        return Err(last_error());
    }
    Ok(())
}

/// Mark the retained file for deletion when its handle closes. This is not
/// evidence that unlink has happened or that retirement is durably complete.
pub(crate) fn delete_on_close(file: &File) -> Result<(), IoError> {
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: deletion uses the exclusively held original staging handle.
    // The caller must never invoke this after successful publication.
    let success = unsafe {
        SetFileInformationByHandle(
            handle(file),
            FileDispositionInfo,
            (&disposition as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if success == 0 {
        return Err(last_error());
    }
    Ok(())
}

#[cfg(feature = "filesystem-journal")]
fn require_regular(file: &File, private: bool) -> Result<(), IoError> {
    let attributes: FILE_ATTRIBUTE_TAG_INFO = query(file)?;
    let standard: FILE_STANDARD_INFO = query(file)?;
    if attributes.FileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
        || standard.Directory
        || standard.DeletePending
        || (private && standard.NumberOfLinks != 1)
    {
        return Err(IoError::Io(ErrorKind::Unsupported));
    }
    qualify_local_ntfs(file)
}

/// Open the exact existing regular child, without truncation or reparse traversal.
#[cfg(feature = "filesystem-journal")]
pub(crate) fn open_existing_write_through(
    parent: &File,
    name: &mut [u16],
) -> Result<File, IoError> {
    if !crate::windows_names::valid_component(name) {
        return Err(invalid());
    }
    let file = relative_file(
        parent,
        name,
        null_mut(),
        FILE_OPEN,
        GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE,
    )
    .map_err(|failure| failure.cause)?;
    require_regular(&file, false)?;
    Ok(file)
}

/// Create an exclusive sidecar with a protected current-user DACL already in
/// effect before any journal data can be written. Caller pays PRIVATE_SECURITY_*.
#[cfg(feature = "filesystem-journal")]
pub(crate) fn create_private_write_through(
    parent: &File,
    name: &mut [u16],
) -> Result<File, PrivateCreateFailure> {
    if !crate::windows_names::valid_component(name) {
        return Err(PrivateCreateFailure {
            cause: invalid(),
            entry_may_remain: false,
            file: None,
        });
    }
    let mut security =
        security::PrivateSecurity::capture().map_err(|cause| PrivateCreateFailure {
            cause,
            entry_may_remain: false,
            file: None,
        })?;
    let file = security
        .with_descriptor(|descriptor| {
            relative_file(
                parent,
                name,
                descriptor,
                FILE_CREATE,
                GENERIC_READ | GENERIC_WRITE | DELETE | SYNCHRONIZE,
            )
        })
        .map_err(|cause| PrivateCreateFailure {
            cause,
            entry_may_remain: false,
            file: None,
        })??;
    retain_private_qualification(file, |file| {
        require_regular(file, true).and_then(|()| security.verify(file))
    })
}

#[cfg(feature = "filesystem-journal")]
fn retain_private_qualification(
    file: File,
    qualify: impl FnOnce(&File) -> Result<(), IoError>,
) -> Result<File, PrivateCreateFailure> {
    match qualify(&file) {
        Ok(()) => Ok(file),
        Err(cause) => Err(PrivateCreateFailure {
            cause,
            entry_may_remain: true,
            file: Some(file),
        }),
    }
}

/// Reopen only an exact ordinary sidecar whose explicit current-user security
/// descriptor still qualifies. Inherited/default/broader DACLs are refused.
#[cfg(feature = "filesystem-journal")]
pub(crate) fn open_private_write_through(parent: &File, name: &mut [u16]) -> Result<File, IoError> {
    if !crate::windows_names::valid_component(name) {
        return Err(invalid());
    }
    let mut security = security::PrivateSecurity::capture()?;
    let file = relative_file(
        parent,
        name,
        null_mut(),
        FILE_OPEN,
        GENERIC_READ | GENERIC_WRITE | DELETE | SYNCHRONIZE,
    )
    .map_err(|failure| failure.cause)?;
    require_regular(&file, true)?;
    security.verify(&file)?;
    Ok(file)
}

/// Acknowledge actual handle close, retaining the owner on failure. The caller
/// must deny all further access after a failed close and must not retry through
/// this API. Drop is then only a best-effort fallback, not a release receipt.
#[cfg(feature = "filesystem-journal")]
pub(crate) fn close_file(file: &mut Option<File>) -> Result<(), IoError> {
    let Some(owned) = file.as_ref() else {
        return Ok(());
    };
    // SAFETY: this is the unique retained live handle. No operation on the File
    // occurs after successful close; into_raw_handle immediately disarms Drop.
    if unsafe { CloseHandle(handle(owned)) } == 0 {
        return Err(last_error());
    }
    if let Some(closed) = file.take() {
        let _ = closed.into_raw_handle();
    }
    Ok(())
}

fn relative_attributes(disposition: u32) -> u32 {
    #[cfg(feature = "filesystem-journal")]
    if disposition == FILE_OPEN {
        return OBJ_CASE_INSENSITIVE;
    }
    let _ = disposition;
    0
}

fn relative_error(status: i32, disposition: u32) -> IoError {
    #[cfg(feature = "filesystem-journal")]
    if disposition == FILE_OPEN {
        // SAFETY: pure status conversion, without borrowed memory.
        let code = unsafe { RtlNtStatusToDosError(status) };
        if matches!(code, ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION) {
            return IoError::LockBusy;
        }
    }
    let _ = disposition;
    nt_error(status)
}

#[cfg(all(test, feature = "filesystem-journal"))]
mod tests;
