//! Checked Windows filesystem calls shared by required publication consumers.
//! Namespace durability policy and resource accounting remain with each caller.
use crate::IoError;
use std::fs::File;
use std::io::ErrorKind;
use std::mem::{offset_of, size_of, size_of_val};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr::{null, null_mut};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_CREATE, FILE_NON_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
    FILE_WRITE_THROUGH, FileFsDeviceInformation, NtCreateFile, NtQueryVolumeInformationFile,
};
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, RtlNtStatusToDosError,
    UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, DELETE, FILE_ADD_FILE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_DISPOSITION_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_RENAME_INFO, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE,
    FileAttributeTagInfo, FileDispositionInfo, FileIdInfo, FileRenameInfo, FlushFileBuffers,
    GetFileInformationByHandleEx, GetVolumeInformationByHandleW, OPEN_EXISTING, SYNCHRONIZE,
    SetFileInformationByHandle,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

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

// FILE_FS_DEVICE_INFORMATION's two ULONG fields are fixed by the Windows ABI.
#[repr(C)]
#[derive(Default)]
struct DeviceInformation {
    device_type: u32,
    characteristics: u32,
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

fn query<T: Default>(file: &File, class: i32) -> Result<T, IoError> {
    let mut information = T::default();
    // SAFETY: information is a correctly aligned writable output of the selected
    // Windows ABI structure. Every call site selects the matching information class.
    let success = unsafe {
        GetFileInformationByHandleEx(
            handle(file),
            class,
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
    let information: FILE_ID_INFO = query(file, FileIdInfo)?;
    Ok(FileIdentity {
        volume: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

pub(crate) fn flush(file: &File) -> Result<(), IoError> {
    // SAFETY: file retains a valid synchronous handle through the callback.
    if unsafe { FlushFileBuffers(handle(file)) } == 0 {
        return Err(last_error());
    }
    Ok(())
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
    let attributes: FILE_ATTRIBUTE_TAG_INFO = query(&parent, FileAttributeTagInfo)?;
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
    let mut device = DeviceInformation::default();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: synchronous directory handle and exact FILE_FS_DEVICE_INFORMATION
    // layout; output lives until the synchronous query completes.
    let result = unsafe {
        NtQueryVolumeInformationFile(
            handle(parent),
            &mut status,
            (&mut device as *mut DeviceInformation).cast(),
            size_of::<DeviceInformation>() as u32,
            FileFsDeviceInformation,
        )
    };
    if result < 0 {
        return Err(nt_error(result));
    }
    if device.device_type != FILE_DEVICE_DISK || device.characteristics & FILE_REMOTE_DEVICE != 0 {
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
        ..Default::default()
    };
    let mut raw = null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // SAFETY: relative validated simple name and retained directory;
    // synchronous FILE_CREATE cannot replace an existing entry. Share 0
    // prevents another handle from replacing or modifying this stage.
    let result = unsafe {
        NtCreateFile(
            &mut raw,
            GENERIC_READ | GENERIC_WRITE | DELETE | SYNCHRONIZE,
            &attributes,
            &mut status,
            null(),
            FILE_ATTRIBUTE_NORMAL,
            0,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE
                | FILE_SYNCHRONOUS_IO_NONALERT
                | FILE_WRITE_THROUGH
                | FILE_OPEN_REPARSE_POINT,
            null(),
            0,
        )
    };
    if result < 0 {
        let error = nt_error(result);
        return Err(CreateFailure {
            cause: error,
            entry_may_remain: false,
        });
    }
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(CreateFailure {
            cause: IoError::Io(ErrorKind::InvalidData),
            entry_may_remain: true,
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
