use super::protocol::{Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError};
use crate::{
    IoError,
    namespace::{HeldDirectory, Location, ObjectIdentity, RelativeName},
    progress::check,
    write_exact_at,
};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use std::ffi::CStr;
use std::fs::File;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use tablecraft_core::limits::{Resource, ResourceBudget};

const STAGE_PREFIX: &[u8] = b".tablecraft-stage-";
const RANDOM_BYTES: usize = 16;
const STAGE_NAME_BYTES: usize = STAGE_PREFIX.len() + RANDOM_BYTES * HEX_DIGITS_PER_BYTE + 1;
const HEX_DIGITS_PER_BYTE: usize = 2;
const HEX_NIBBLE_BITS: u8 = 4;
const HEX_NIBBLE_MASK: u8 = 0x0f;
const HEX: &[u8; 16] = b"0123456789abcdef";
const MAX_STAGE_ATTEMPTS: usize = 16;
const CREATE_CALLBACK_STEPS: u64 = 6;
const WRITE_CHUNK_BYTES: usize = 64 * 1024;
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
const PERMISSION_BITS: u32 = 0o777;
const PAYLOAD: &CStr = c"payload";

#[cfg(test)]
type TestEntropy = fn(&mut [u8; RANDOM_BYTES]) -> Result<(), IoError>;

pub(super) struct Unix {
    parent: HeldDirectory,
    identity: ParentIdentity,
    final_name: RelativeName,
    #[cfg(test)]
    entropy_override: Option<TestEntropy>,
}

pub(super) struct Stage {
    directory: HeldDirectory,
    identity: ObjectIdentity,
    name: [u8; STAGE_NAME_BYTES],
    payload: Option<(File, ObjectIdentity)>,
}

fn io(error: rustix::io::Errno) -> IoError {
    IoError::from(std::io::Error::from(error))
}

impl Stage {
    fn name(&self) -> &CStr {
        // All bytes before the final NUL are fixed ASCII or hex.
        CStr::from_bytes_with_nul(&self.name).expect("fixed ASCII staging component")
    }
}

impl Unix {
    fn entropy(&self, bytes: &mut [u8; RANDOM_BYTES]) -> Result<(), IoError> {
        #[cfg(test)]
        if let Some(callback) = self.entropy_override {
            return callback(bytes);
        }
        getrandom::fill(bytes).map_err(|_| IoError::Io(ErrorKind::Other))
    }

    pub(super) fn capture(
        output: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PublicationError> {
        let mut prepare = || -> Result<Self, IoError> {
            let (parent, final_name) = Location::capture(output, budget, cancelled)?.into_parts();
            let observed = parent.identity();
            let identity = ParentIdentity {
                device: observed.device,
                inode: observed.inode,
            };

            // Discover unsupported directory barriers before creating any entry.
            check(budget, cancelled)?;
            parent.sync()?;
            Ok(Self {
                parent,
                identity,
                final_name,
                #[cfg(test)]
                entropy_override: None,
            })
        };
        prepare().map_err(PublicationError::before)
    }
}

impl Backend for Unix {
    type Stage = Stage;

    fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    fn terminal_work(&self) -> u64 {
        // Borrowed CStr validation scans this paid, already captured buffer;
        // linkat receives CStr directly and performs no Arg CString allocation.
        self.final_name.validation_work()
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
        for _ in 0..MAX_STAGE_ATTEMPTS {
            check(budget, cancelled).map_err(before)?;
            budget
                .charge(Resource::Steps, CREATE_CALLBACK_STEPS)
                .map_err(IoError::from)
                .map_err(before)?;
            let mut random = [0; RANDOM_BYTES];
            self.entropy(&mut random).map_err(before)?;
            let mut name = [0; STAGE_NAME_BYTES];
            name[..STAGE_PREFIX.len()].copy_from_slice(STAGE_PREFIX);
            for (index, byte) in random.iter().enumerate() {
                let offset = STAGE_PREFIX.len() + index * HEX_DIGITS_PER_BYTE;
                name[offset] = HEX[(byte >> HEX_NIBBLE_BITS) as usize];
                name[offset + 1] = HEX[(byte & HEX_NIBBLE_MASK) as usize];
            }
            let component =
                CStr::from_bytes_with_nul(&name).expect("fixed ASCII staging component");
            // The stage must never occupy the caller's final component, even
            // when an injected or extraordinarily unlucky nonce matches it.
            if component.to_bytes() == self.final_name.as_bytes() {
                continue;
            }
            match fs::mkdirat(&self.parent, component, Mode::RWXU) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(before(io(error))),
            }

            // After mkdir, a failed bind cannot safely clean an unbound name.
            let retained = |cause| CreateFailure {
                cause,
                temporary_may_remain: true,
            };
            let directory = File::from(
                fs::openat(
                    &self.parent,
                    component,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io)
                .map_err(retained)?,
            );
            let metadata = directory
                .metadata()
                .map_err(IoError::from)
                .map_err(retained)?;
            if !metadata.is_dir()
                || metadata.mode() & PERMISSION_BITS != PRIVATE_DIRECTORY_MODE
                || metadata.uid() != rustix::process::geteuid().as_raw()
            {
                return Err(retained(IoError::Io(ErrorKind::PermissionDenied)));
            }
            let identity = ObjectIdentity::from_metadata(&metadata);
            let directory =
                HeldDirectory::adopt_observed(directory, &metadata).map_err(retained)?;
            self.parent
                .verify_child(component, identity)
                .map_err(retained)?;
            return Ok(Stage {
                directory,
                identity,
                name,
                payload: None,
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
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        let file = File::from(
            fs::openat(
                &stage.directory,
                PAYLOAD,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(io)?,
        );
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        let payload_identity = ObjectIdentity::from_metadata(&metadata);
        stage.payload = Some((file, payload_identity));
        let file = &mut stage.payload.as_mut().expect("bound payload").0;
        for (index, chunk) in bytes.chunks(WRITE_CHUNK_BYTES).enumerate() {
            write_exact_at(
                file,
                (index * WRITE_CHUNK_BYTES) as u64,
                chunk,
                budget,
                cancelled,
            )?;
        }
        Ok(())
    }

    fn sync_payload(&self, stage: &Stage) -> Result<(), IoError> {
        let (file, _) = stage
            .payload
            .as_ref()
            .ok_or(IoError::Io(ErrorKind::InvalidData))?;
        file.sync_all().map_err(IoError::from)
    }

    fn verify(&self, stage: &Stage) -> Result<(), IoError> {
        self.parent.verify_child(stage.name(), stage.identity)?;
        let (_, payload) = stage
            .payload
            .as_ref()
            .ok_or(IoError::Io(ErrorKind::InvalidData))?;
        stage.directory.verify_child(PAYLOAD, *payload)
    }

    fn publish(&self, stage: &Stage) -> Result<(), PublishFailure> {
        match fs::linkat(
            &stage.directory,
            PAYLOAD,
            &self.parent,
            self.final_name.as_c_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::EXIST) => Err(PublishFailure::Collision(IoError::Io(
                ErrorKind::AlreadyExists,
            ))),
            Err(error) => Err(PublishFailure::Uncertain(io(error))),
        }
    }

    fn cleanup(&self, stage: &Stage) -> Result<(), IoError> {
        if let Some((_, expected)) = &stage.payload {
            stage.directory.verify_child(PAYLOAD, *expected)?;
            fs::unlinkat(&stage.directory, PAYLOAD, AtFlags::empty()).map_err(io)?;
        } else {
            match fs::statat(&stage.directory, PAYLOAD, AtFlags::SYMLINK_NOFOLLOW) {
                Err(rustix::io::Errno::NOENT) => {}
                Ok(_) => return Err(IoError::Io(ErrorKind::InvalidData)),
                Err(error) => return Err(io(error)),
            }
        }
        self.parent.verify_child(stage.name(), stage.identity)?;
        fs::unlinkat(&self.parent, stage.name(), AtFlags::REMOVEDIR).map_err(io)
    }

    fn sync_parent(&self) -> Result<(), IoError> {
        self.parent.sync()
    }
}

#[cfg(test)]
mod tests;
