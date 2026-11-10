//! Publish one private flat sidecar before native target mutation is available.
use super::{OfflineTarget, PairError, binding_identity, storage::SidecarStorage};
use crate::{
    IoError,
    journal::{
        flat_binding::{self, FlatBindingData, FlatObjects},
        storage::BLOCK_BYTES,
    },
    progress::check,
    read_exact_at,
    windows_fs::{self as filesystem, Location},
    write_exact_at,
};
use std::{fs::File, io::ErrorKind, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NONCE_BYTES: usize = 16;
const HEX_PER_BYTE: usize = 2;
const NIBBLE_BITS: u8 = 4;
const NIBBLE_MASK: u8 = 0x0f;
const HEX: &[u8; 16] = b"0123456789abcdef";
const STAGE_PREFIX: &str = ".tablecraft-journal-stage-";
const RETIRED_PREFIX: &str = ".tablecraft-retired-";
const MAX_CREATE_ATTEMPTS: usize = 16;
const MAX_INTENT_BYTES: u64 = 16 * 1024 * 1024;
const SETUP_RENAME_STEPS: u64 = 1024;
pub(super) const RETIRED_UNITS: usize = RETIRED_PREFIX.len() + NONCE_BYTES * HEX_PER_BYTE;
const STAGE_UNITS: usize = STAGE_PREFIX.len() + NONCE_BYTES * HEX_PER_BYTE;

#[derive(Clone, Copy, Debug)]
pub(super) enum SetupPhase {
    Captured,
    CreateAttempted,
    Created,
    PrefixWritten,
    PrefixSynced,
    RenameAttempted,
    Renamed,
    Published,
}

pub(super) struct Sidecar {
    pub(super) parent: Option<File>,
    pub(super) name: Vec<u16>,
    pub(super) storage: SidecarStorage,
    pub(super) binding: Option<FlatBindingData>,
    pub(super) intent: Vec<u8>,
    pub(super) retired_name: [u16; RETIRED_UNITS],
    pub(super) stage_name: [u16; STAGE_UNITS],
    pub(super) phase: SetupPhase,
}

impl Sidecar {
    fn captured(location: Location, intent: Vec<u8>) -> Self {
        Self {
            parent: Some(location.parent),
            name: location.name,
            storage: SidecarStorage { file: None },
            binding: None,
            intent,
            retired_name: [0; RETIRED_UNITS],
            stage_name: [0; STAGE_UNITS],
            phase: SetupPhase::Captured,
        }
    }

    pub(super) fn parent(&self) -> Result<&File, PairError> {
        self.parent.as_ref().ok_or(PairError::InvalidState)
    }

    pub(super) fn file(&self) -> Result<&File, PairError> {
        self.storage.file.as_ref().ok_or(PairError::InvalidState)
    }

    pub(super) fn binding(&self) -> Result<&FlatBindingData, PairError> {
        self.binding.as_ref().ok_or(PairError::InvalidState)
    }

    pub(super) fn require_objects(
        &self,
        target: &OfflineTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), PairError> {
        target.require_entry(budget, cancelled)?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 2).map_err(IoError::from)?;
        let actual = FlatObjects {
            target: binding_identity(target.identity()),
            target_parent: binding_identity(target.parent_identity()),
            sidecar: binding_identity(filesystem::identity(self.file()?)?),
            sidecar_parent: binding_identity(filesystem::identity(self.parent()?)?),
        };
        self.binding()?.verify_objects(&actual, budget, cancelled)?;
        Ok(())
    }

    pub(super) fn close_resources(&mut self) -> Result<(), PairError> {
        filesystem::close_file(&mut self.storage.file)?;
        filesystem::close_file(&mut self.parent)?;
        Ok(())
    }
}

pub(super) struct FailedSetup {
    pub(super) cause: PairError,
    pub(super) sidecar: Option<Sidecar>,
}

fn component<const N: usize>(prefix: &str, nonce: &[u8; NONCE_BYTES]) -> [u16; N] {
    let mut name = [0; N];
    for (destination, byte) in name.iter_mut().zip(prefix.bytes()) {
        *destination = u16::from(byte);
    }
    for (position, byte) in nonce.iter().enumerate() {
        let offset = prefix.len() + position * HEX_PER_BYTE;
        name[offset] = u16::from(HEX[usize::from(byte >> NIBBLE_BITS)]);
        name[offset + 1] = u16::from(HEX[usize::from(byte & NIBBLE_MASK)]);
    }
    name
}

fn same_reserved_name(left: &[u16], right: &[u16]) -> bool {
    let fold = |unit: u16| {
        if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
            unit + u16::from(b'a' - b'A')
        } else {
            unit
        }
    };
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| fold(*left) == fold(*right))
}

fn paid_write(
    file: &mut File,
    offset: u64,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PairError> {
    for (position, chunk) in bytes.chunks(BLOCK_BYTES).enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Bytes, chunk.len() as u64)
            .map_err(IoError::from)?;
        write_exact_at(
            file,
            offset + (position * BLOCK_BYTES) as u64,
            chunk,
            budget,
            cancelled,
        )?;
    }
    Ok(())
}

#[allow(clippy::result_large_err)] // Preserve partial namespace ownership without error-path allocation.
pub(super) fn create(
    target: &mut OfflineTarget,
    path: &Path,
    intent: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Sidecar, FailedSetup> {
    let mut retained = None;
    let result = (|| -> Result<(), PairError> {
        check(budget, cancelled)?;
        if intent.len() as u64 > MAX_INTENT_BYTES {
            return Err(PairError::IntentLimit);
        }
        let location = Location::capture(path, budget, cancelled)?;
        retained = Some(Sidecar::captured(location, intent));
        let sidecar = retained.as_mut().ok_or(PairError::InvalidState)?;
        target.sync_baseline(budget, cancelled)?;

        let mut nonce = [0; NONCE_BYTES];
        for _ in 0..MAX_CREATE_ATTEMPTS {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (NONCE_BYTES + STAGE_UNITS * 4) as u64)
                .map_err(IoError::from)?;
            budget
                .charge(Resource::Bytes, filesystem::PRIVATE_SECURITY_BYTES)
                .map_err(IoError::from)?;
            budget
                .charge(Resource::Steps, filesystem::PRIVATE_SECURITY_STEPS)
                .map_err(IoError::from)?;
            getrandom::fill(&mut nonce).map_err(|_| IoError::Io(ErrorKind::Other))?;
            sidecar.stage_name = component(STAGE_PREFIX, &nonce);
            sidecar.retired_name = component(RETIRED_PREFIX, &nonce);
            if same_reserved_name(&sidecar.name, &sidecar.stage_name)
                || same_reserved_name(&sidecar.name, &sidecar.retired_name)
            {
                continue;
            }
            sidecar.phase = SetupPhase::CreateAttempted;
            let parent = sidecar.parent.as_ref().ok_or(PairError::InvalidState)?;
            match filesystem::create_private_write_through(parent, &mut sidecar.stage_name) {
                Ok(file) => {
                    sidecar.storage.file = Some(file);
                    sidecar.phase = SetupPhase::Created;
                    break;
                }
                Err(failure)
                    if !failure.entry_may_remain
                        && failure.cause == IoError::Io(ErrorKind::AlreadyExists) => {}
                Err(mut failure) => {
                    if let Some(file) = failure.file.take() {
                        sidecar.storage.file = Some(file);
                        sidecar.phase = SetupPhase::Created;
                    }
                    return Err(failure.cause.into());
                }
            }
        }
        if sidecar.storage.file.is_none() {
            return Err(IoError::Io(ErrorKind::AlreadyExists).into());
        }
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 2).map_err(IoError::from)?;
        let objects = FlatObjects {
            target: binding_identity(target.identity()),
            target_parent: binding_identity(target.parent_identity()),
            sidecar: binding_identity(filesystem::identity(sidecar.file()?)?),
            sidecar_parent: binding_identity(filesystem::identity(sidecar.parent()?)?),
        };
        let binding = FlatBindingData::capture(
            nonce,
            objects,
            &sidecar.intent,
            MAX_INTENT_BYTES,
            budget,
            cancelled,
        )?;
        let header = binding.encode(MAX_INTENT_BYTES, budget, cancelled)?;
        sidecar.binding = Some(binding);
        paid_write(sidecar.storage.file_mut()?, 0, &header, budget, cancelled)?;
        let Sidecar {
            storage, intent, ..
        } = sidecar;
        paid_write(
            storage.file_mut()?,
            flat_binding::HEADER_BYTES as u64,
            intent,
            budget,
            cancelled,
        )?;
        sidecar.phase = SetupPhase::PrefixWritten;
        check(budget, cancelled)?;
        filesystem::flush(sidecar.file()?)?;
        sidecar.phase = SetupPhase::PrefixSynced;
        check(budget, cancelled)?;
        target.require_entry(budget, cancelled)?;
        sidecar.require_objects(target, budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                SETUP_RENAME_STEPS + sidecar.name.len() as u64,
            )
            .map_err(IoError::from)?;
        check(budget, cancelled)?;
        sidecar.phase = SetupPhase::RenameAttempted;
        filesystem::rename_no_replace(sidecar.file()?, sidecar.parent()?, &sidecar.name)?;
        sidecar.phase = SetupPhase::Renamed;
        check(budget, cancelled)?;
        filesystem::flush(sidecar.file()?)?;
        sidecar.phase = SetupPhase::Published;
        check(budget, cancelled)?;
        Ok(())
    })();
    match result {
        Ok(()) => retained.ok_or(FailedSetup {
            cause: PairError::InvalidState,
            sidecar: None,
        }),
        Err(cause) => Err(FailedSetup {
            cause,
            sidecar: retained,
        }),
    }
}

#[allow(clippy::result_large_err)] // Preserve every opened owner without error-path allocation.
pub(super) fn reopen(
    target: &OfflineTarget,
    path: &Path,
    maximum_intent_bytes: u64,
    retired: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Sidecar, FailedSetup> {
    let mut retained = None;
    let result = (|| -> Result<(), PairError> {
        let location = Location::capture(path, budget, cancelled)?;
        retained = Some(Sidecar::captured(location, Vec::new()));
        let sidecar = retained.as_mut().ok_or(PairError::InvalidState)?;
        check(budget, cancelled)?;
        budget
            .charge(Resource::Bytes, filesystem::PRIVATE_SECURITY_BYTES)
            .map_err(IoError::from)?;
        budget
            .charge(Resource::Steps, filesystem::PRIVATE_SECURITY_STEPS)
            .map_err(IoError::from)?;
        let parent = sidecar.parent.as_ref().ok_or(PairError::InvalidState)?;
        let file = filesystem::open_private_write_through(parent, &mut sidecar.name)?;
        sidecar.storage.file = Some(file);
        sidecar.phase = SetupPhase::Created;
        target.require_entry(budget, cancelled)?;

        let mut header = [0; flat_binding::HEADER_BYTES];
        budget
            .charge(Resource::Bytes, header.len() as u64)
            .map_err(IoError::from)?;
        read_exact_at(
            sidecar.storage.file_mut()?,
            0,
            &mut header,
            budget,
            cancelled,
        )?;
        let binding = flat_binding::decode(&header, maximum_intent_bytes, budget, cancelled)?;
        sidecar.retired_name = component(RETIRED_PREFIX, &binding.nonce);
        budget
            .charge(Resource::Steps, (sidecar.name.len() + RETIRED_UNITS) as u64)
            .map_err(IoError::from)?;
        if retired != same_reserved_name(&sidecar.name, &sidecar.retired_name) {
            return Err(PairError::ReservedName);
        }
        let prefix = binding.journal_offset()?;
        check(budget, cancelled)?;
        let length = sidecar.file()?.metadata().map_err(IoError::from)?.len();
        if length < prefix {
            return Err(IoError::Io(ErrorKind::InvalidData).into());
        }
        let count = usize::try_from(binding.intent_length).map_err(|_| PairError::IntentLimit)?;
        sidecar.binding = Some(binding);
        // Reject a different physical target or sidecar before allocating or
        // reading its variable-sized intent. Stored identities grant no opens.
        sidecar.require_objects(target, budget, cancelled)?;
        budget
            .charge_allocation(count as u64, 1)
            .map_err(IoError::from)?;
        sidecar
            .intent
            .try_reserve_exact(count)
            .map_err(|_| IoError::Io(ErrorKind::OutOfMemory))?;
        sidecar.intent.resize(count, 0);
        let Sidecar {
            storage, intent, ..
        } = sidecar;
        for (position, chunk) in intent.chunks_mut(BLOCK_BYTES).enumerate() {
            budget
                .charge(Resource::Bytes, chunk.len() as u64)
                .map_err(IoError::from)?;
            read_exact_at(
                storage.file_mut()?,
                flat_binding::HEADER_BYTES as u64 + (position * BLOCK_BYTES) as u64,
                chunk,
                budget,
                cancelled,
            )?;
        }
        sidecar
            .binding()?
            .verify_intent(&sidecar.intent, budget, cancelled)?;
        sidecar.phase = SetupPhase::Published;
        Ok(())
    })();
    match result {
        Ok(()) => retained.ok_or(FailedSetup {
            cause: PairError::InvalidState,
            sidecar: None,
        }),
        Err(cause) => Err(FailedSetup {
            cause,
            sidecar: retained,
        }),
    }
}
