//! Local NTFS publication through retained directory and write-through file handles.
//! No pathname-based replacement or cleanup is performed after capture.

use super::protocol::{Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError, PublicationGuarantee, windows_names as names};
use crate::{IoError, progress::check, windows_fs as filesystem, write_exact_at};
use std::cell::Cell;
use std::fs::File;
use std::io::ErrorKind;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, Prefix};
use tablecraft_core::limits::{Resource, ResourceBudget};
use windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO;

const MAX_COMPONENT_UNITS: usize = filesystem::MAX_COMPONENT_UNITS;
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
const HEX: &[u8; 16] = b"0123456789abcdef";

pub(super) struct Windows {
    parent: File,
    identity: ParentIdentity,
    final_name: Vec<u16>,
}

pub(super) struct Stage {
    file: File,
    identity: filesystem::FileIdentity,
    renamed: Cell<bool>,
}

fn invalid() -> IoError {
    IoError::Io(ErrorKind::InvalidInput)
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

fn parent_identity(file: &File) -> Result<ParentIdentity, IoError> {
    let identity = filesystem::identity(file)?;
    Ok(ParentIdentity {
        volume: identity.volume,
        file_id: identity.file_id,
    })
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
            let parent = filesystem::open_parent(&path)?;
            filesystem::qualify_local_ntfs(&parent)?;
            let identity = parent_identity(&parent)?;
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
            let file = match filesystem::create_exclusive_write_through(&self.parent, &mut name) {
                Ok(file) => file,
                Err(failure) => {
                    if !failure.entry_may_remain
                        && failure.cause == IoError::Io(ErrorKind::AlreadyExists)
                    {
                        continue;
                    }
                    return Err(CreateFailure {
                        cause: failure.cause,
                        temporary_may_remain: failure.entry_may_remain,
                    });
                }
            };
            let identity = match filesystem::identity(&file) {
                Ok(identity) => identity,
                Err(cause) => {
                    let temporary_may_remain = filesystem::delete_on_close(&file).is_err();
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
        filesystem::flush(&stage.file)
    }

    fn verify(&self, stage: &Stage) -> Result<(), IoError> {
        if filesystem::identity(&stage.file)? != stage.identity
            || parent_identity(&self.parent)? != self.identity
        {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        Ok(())
    }

    fn publish(&self, stage: &Stage) -> Result<(), PublishFailure> {
        if let Err(error) =
            filesystem::rename_no_replace(&stage.file, &self.parent, &self.final_name)
        {
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
        filesystem::delete_on_close(&stage.file)
    }

    fn sync_namespace(&self, stage: &Stage) -> Result<(), IoError> {
        filesystem::flush(&stage.file)
    }
}
