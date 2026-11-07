//! Local NTFS publication through retained directory and write-through file handles.
//! No pathname-based replacement or cleanup is performed after capture.

use super::protocol::{Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError, PublicationGuarantee, windows_names as names};
use crate::{IoError, progress::check, write_exact_at};
use std::cell::Cell;
use std::fs::File;
use std::io::ErrorKind;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path, Prefix};
use std::ptr::{null, null_mut};
use tablecraft_core::limits::{Resource, ResourceBudget};
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

const MAX_COMPONENT_UNITS: usize = 255;
const MAX_PATH_UNITS: usize = 32_767;
const PATH_SCAN_PASSES: u64 = 12;
const RANDOM_BYTES: usize = 16;
const STAGE_PREFIX: &str = ".tablecraft-stage-";
const HEX_DIGITS_PER_BYTE: usize = 2;
const HEX_NIBBLE_BITS: u8 = 4;
const HEX_NIBBLE_MASK: u8 = 0x0f;
const STAGE_NAME_UNITS: usize = STAGE_PREFIX.len() + RANDOM_BYTES * HEX_DIGITS_PER_BYTE;
const CREATE_CALLBACK_STEPS: u64 = 4 + STAGE_NAME_UNITS as u64 * 4 + RANDOM_BYTES as u64 * 2;
const STAGE_ATTEMPTS: usize = 16;
const WRITE_CHUNK_BYTES: usize = 64 * 1024;
const FILE_REMOTE_DEVICE: u32 = 0x10;
const FILE_DEVICE_DISK: u32 = 7;
const HEX: &[u8; 16] = b"0123456789abcdef";

// FILE_FS_DEVICE_INFORMATION's two ULONG fields are fixed by the Windows ABI.
#[repr(C)]
#[derive(Default)]
struct DeviceInformation {
    device_type: u32,
    characteristics: u32,
}

pub(super) struct Windows {
    parent: File,
    identity: ParentIdentity,
    final_name: Vec<u16>,
}

pub(super) struct Stage {
    file: File,
    identity: ParentIdentity,
    renamed: Cell<bool>,
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

fn bounded_wide(
    value: &std::ffi::OsStr,
    maximum: usize,
    terminated: bool,
) -> Result<Vec<u16>, IoError> {
    let length = value.encode_wide().take(maximum + 1).count();
    if length > maximum {
        return Err(invalid());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(length + usize::from(terminated))
        .map_err(|_| IoError::Io(ErrorKind::OutOfMemory))?;
    result.extend(value.encode_wide());
    if terminated {
        result.push(0);
    }
    Ok(result)
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

fn identity(file: &File) -> Result<ParentIdentity, IoError> {
    let information: FILE_ID_INFO = query(file, FileIdInfo)?;
    Ok(ParentIdentity {
        volume: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

fn flush(file: &File) -> Result<(), IoError> {
    // SAFETY: file retains a valid synchronous handle through the callback.
    if unsafe { FlushFileBuffers(handle(file)) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

impl Windows {
    pub(super) fn capture(
        output: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PublicationError> {
        let prepare = || -> Result<Self, IoError> {
            check(budget, cancelled)?;
            let input_length = output
                .as_os_str()
                .encode_wide()
                .take(MAX_PATH_UNITS + 1)
                .count();
            budget.charge(Resource::Steps, input_length as u64)?;
            if input_length > MAX_PATH_UNITS {
                return Err(invalid());
            }
            if matches!(output.as_os_str().encode_wide().last(), Some(47 | 92)) {
                return Err(invalid());
            }
            // Fully relative names resolve once. Drive-relative and root-relative
            // spellings depend on hidden per-drive state and remain refused.
            if !output.is_absolute()
                && output
                    .components()
                    .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
            {
                return Err(invalid());
            }
            // Validate the raw leaf before Win32 absolute-path normalization can
            // alter trailing dots/spaces or interpret a DOS device spelling.
            let mut raw_leaf = [0; MAX_COMPONENT_UNITS];
            let mut raw_leaf_length = 0;
            for unit in output.as_os_str().encode_wide() {
                if unit == b'\\' as u16 || unit == b'/' as u16 {
                    raw_leaf_length = 0;
                    continue;
                }
                if raw_leaf_length == raw_leaf.len() {
                    return Err(invalid());
                }
                raw_leaf[raw_leaf_length] = unit;
                raw_leaf_length += 1;
            }
            if !names::valid_component(&raw_leaf[..raw_leaf_length]) {
                return Err(invalid());
            }
            // Windows resolution can include the process current directory.
            // Admit its platform maximum before invoking the allocating OS helper.
            budget.charge(
                Resource::Bytes,
                ((MAX_PATH_UNITS + 1) * size_of::<u16>()) as u64,
            )?;
            budget.charge(Resource::Steps, MAX_PATH_UNITS as u64)?;
            let resolved = std::path::absolute(output)?;
            let output = resolved.as_path();
            let length = output
                .as_os_str()
                .encode_wide()
                .take(MAX_PATH_UNITS + 1)
                .count();
            budget.charge(Resource::Steps, length as u64 * PATH_SCAN_PASSES)?;
            budget.charge(
                Resource::Bytes,
                (length * size_of::<u16>() * 2 + size_of::<u16>()) as u64,
            )?;
            if length > MAX_PATH_UNITS || !output.is_absolute() {
                return Err(invalid());
            }
            match output.components().next() {
                Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)) => {}
                _ => return Err(invalid()),
            }
            for component in output.components() {
                match component {
                    Component::Normal(name) => {
                        let mut component = [0; MAX_COMPONENT_UNITS];
                        let mut count = 0;
                        for unit in name.encode_wide() {
                            if count == component.len() {
                                return Err(invalid());
                            }
                            component[count] = unit;
                            count += 1;
                        }
                        if !names::valid_component(&component[..count]) {
                            return Err(invalid());
                        }
                    }
                    Component::Prefix(_) | Component::RootDir => {}
                    _ => return Err(invalid()),
                }
            }
            let final_name = bounded_wide(
                output.file_name().ok_or_else(invalid)?,
                MAX_COMPONENT_UNITS,
                false,
            )?;
            if !names::valid_component(&final_name) {
                return Err(invalid());
            }
            let parent_path = output.parent().ok_or_else(invalid)?;
            let path = bounded_wide(parent_path.as_os_str(), MAX_PATH_UNITS, true)?;
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
            qualify_ntfs(&parent)?;
            let identity = identity(&parent)?;
            Ok(Self {
                parent,
                identity,
                final_name,
            })
        };
        let mut prepare = prepare;
        prepare().map_err(|cause| {
            if cause == IoError::Io(ErrorKind::Unsupported) {
                PublicationError::UnsupportedDurability
            } else {
                PublicationError::before(cause)
            }
        })
    }
}

fn qualify_ntfs(parent: &File) -> Result<(), IoError> {
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

impl Backend for Windows {
    type Stage = Stage;

    fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    fn guarantee(&self) -> PublicationGuarantee {
        PublicationGuarantee::NtfsWriteThroughRename
    }

    fn terminal_work(&self) -> u64 {
        // Fixed rename-buffer initialization plus bounded UTF-16 copy. Reserve
        // this before publication, when caller cancellation/budgets still apply.
        (size_of::<FILE_RENAME_INFO>() + MAX_COMPONENT_UNITS * size_of::<u16>()) as u64
            + self.final_name.len() as u64
    }

    fn create(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Stage, CreateFailure> {
        let before = |cause| CreateFailure {
            cause,
            temporary_may_remain: false,
        };
        for _ in 0..STAGE_ATTEMPTS {
            check(budget, cancelled).map_err(before)?;
            budget
                .charge(Resource::Steps, CREATE_CALLBACK_STEPS)
                .map_err(IoError::from)
                .map_err(before)?;
            let mut random = [0; RANDOM_BYTES];
            getrandom::fill(&mut random).map_err(|_| before(IoError::Io(ErrorKind::Other)))?;
            let mut name = [0; STAGE_NAME_UNITS];
            for (destination, unit) in name.iter_mut().zip(STAGE_PREFIX.encode_utf16()) {
                *destination = unit;
            }
            for (index, byte) in random.iter().enumerate() {
                let offset = STAGE_PREFIX.len() + index * HEX_DIGITS_PER_BYTE;
                name[offset] = HEX[(byte >> HEX_NIBBLE_BITS) as usize] as u16;
                name[offset + 1] = HEX[(byte & HEX_NIBBLE_MASK) as usize] as u16;
            }
            if name.len() == self.final_name.len()
                && name.iter().zip(&self.final_name).all(|(left, right)| {
                    let folded = if (b'A' as u16..=b'Z' as u16).contains(right) {
                        *right + (b'a' - b'A') as u16
                    } else {
                        *right
                    };
                    *left == folded
                })
            {
                continue;
            }
            let mut unicode = UNICODE_STRING {
                Length: (name.len() * size_of::<u16>()) as u16,
                MaximumLength: (name.len() * size_of::<u16>()) as u16,
                Buffer: name.as_mut_ptr(),
            };
            let attributes = OBJECT_ATTRIBUTES {
                Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
                RootDirectory: handle(&self.parent),
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
                if error == IoError::Io(ErrorKind::AlreadyExists) {
                    continue;
                }
                return Err(before(error));
            }
            if raw.is_null() || raw == INVALID_HANDLE_VALUE {
                return Err(CreateFailure {
                    cause: IoError::Io(ErrorKind::InvalidData),
                    temporary_may_remain: true,
                });
            }
            // SAFETY: NtCreateFile succeeded and transfers unique handle ownership.
            let file = unsafe { File::from_raw_handle(raw) };
            let identity = match identity(&file) {
                Ok(identity) => identity,
                Err(cause) => {
                    let temporary_may_remain = delete_stage(&file).is_err();
                    return Err(CreateFailure {
                        cause,
                        temporary_may_remain,
                    });
                }
            };
            return Ok(Stage {
                file,
                identity,
                renamed: Cell::new(false),
            });
        }
        Err(before(IoError::Io(ErrorKind::AlreadyExists)))
    }

    fn write(
        &self,
        stage: &mut Stage,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        for (index, chunk) in bytes.chunks(WRITE_CHUNK_BYTES).enumerate() {
            write_exact_at(
                &mut stage.file,
                (index * WRITE_CHUNK_BYTES) as u64,
                chunk,
                budget,
                cancelled,
            )?;
        }
        Ok(())
    }

    fn sync_payload(&self, stage: &Stage) -> Result<(), IoError> {
        flush(&stage.file)
    }

    fn verify(&self, stage: &Stage) -> Result<(), IoError> {
        if identity(&stage.file)? != stage.identity || identity(&self.parent)? != self.identity {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        Ok(())
    }

    fn publish(&self, stage: &Stage) -> Result<(), PublishFailure> {
        // Fixed maximum buffer retains FILE_RENAME_INFO alignment and avoids
        // any allocation beyond the visibility boundary. Only initialized bytes
        // through the supplied UTF-16 name are passed to Windows.
        let mut storage = [0usize;
            (size_of::<FILE_RENAME_INFO>() + MAX_COMPONENT_UNITS * size_of::<u16>())
                .div_ceil(size_of::<usize>())];
        let header = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        let name_bytes = self.final_name.len() * size_of::<u16>();
        let length = offset_of!(FILE_RENAME_INFO, FileName) + name_bytes;
        // SAFETY: storage is sufficiently sized/aligned for header plus bounded
        // name. RootDirectory remains alive and ReplaceIfExists stays false.
        let success = unsafe {
            (*header).Anonymous.ReplaceIfExists = false;
            (*header).RootDirectory = handle(&self.parent);
            (*header).FileNameLength = name_bytes as u32;
            std::ptr::copy_nonoverlapping(
                self.final_name.as_ptr(),
                storage
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(offset_of!(FILE_RENAME_INFO, FileName))
                    .cast::<u16>(),
                self.final_name.len(),
            );
            SetFileInformationByHandle(
                handle(&stage.file),
                FileRenameInfo,
                header.cast(),
                length as u32,
            )
        };
        if success == 0 {
            let error = last_error();
            if error == IoError::Io(ErrorKind::AlreadyExists) {
                return Err(PublishFailure::Collision(error));
            }
            return Err(PublishFailure::Uncertain(error));
        }
        stage.renamed.set(true);
        Ok(())
    }

    fn cleanup(&self, stage: &Stage) -> Result<(), IoError> {
        if stage.renamed.get() {
            return Ok(());
        }
        delete_stage(&stage.file)
    }

    fn sync_namespace(&self, stage: &Stage) -> Result<(), IoError> {
        flush(&stage.file)
    }
}

fn delete_stage(file: &File) -> Result<(), IoError> {
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
