//! Local NTFS publication through retained directory and write-through file handles.
//! No pathname-based replacement or cleanup is performed after capture.

use super::protocol::{Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError, PublicationGuarantee};
use crate::{IoError, progress::check, windows_fs as filesystem, write_exact_at};
use std::cell::Cell;
use std::fs::File;
use std::io::ErrorKind;
use std::mem::size_of;
use std::path::Path;
use tablecraft_core::limits::{Resource, ResourceBudget};
use windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO;

const MAX_COMPONENT_UNITS: usize = filesystem::MAX_COMPONENT_UNITS;
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
            let location = filesystem::Location::capture(output, budget, cancelled)?;
            Ok(Self {
                parent: location.parent,
                identity: ParentIdentity {
                    volume: location.identity.volume,
                    file_id: location.identity.file_id,
                },
                final_name: location.name,
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
