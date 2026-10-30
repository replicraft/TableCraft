//! Reopen an explicitly selected existing bundle using retained relative handles.
use super::{
    BINDING_ENTRY, INTENT_ENTRY, JOURNAL_ENTRY,
    binding::{self, BindingData, BindingError, PairObjects, RECORD_BYTES},
    target::OfflineTarget,
};
use crate::{
    ExclusiveFile, IoError,
    journal::storage::BLOCK_BYTES,
    namespace::{
        HeldDirectory, Location, ObjectIdentity, RelativeName, require_private_directory,
        require_private_file,
    },
    progress::check,
    read_exact_at,
};
use rustix::fs::{self, Mode, OFlags};
use std::{ffi::CStr, fs::File, io, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

/// Source acquisition errors do not write a target or a bundle entry.
#[derive(Debug, Error)]
pub(super) enum ReopenError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Binding(#[from] BindingError),
    #[error("retained journal objects do not match the selected pair")]
    IdentityMismatch,
    #[error("retained binding record has an unsupported length")]
    BindingLength,
    #[error("retained intent length or digest does not match")]
    IntentMismatch,
    #[error("retained intent exceeds the caller's byte ceiling")]
    IntentLimit,
}

/// Actual retained objects, assembled only after identity and intent checks.
/// These fields stay inside the filesystem owner; no public raw handles escape.
pub(super) struct ExistingBundle {
    pub _parent: HeldDirectory,
    pub name: RelativeName,
    pub _directory: HeldDirectory,
    pub _binding_file: File,
    pub _intent_file: File,
    pub journal: ExclusiveFile,
    pub binding: BindingData,
    pub intent_bytes: Vec<u8>,
}

fn open_child(
    directory: &HeldDirectory,
    name: &CStr,
    writable: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(File, ObjectIdentity, u64), IoError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    let access = if writable {
        OFlags::RDWR
    } else {
        OFlags::RDONLY
    };
    let file = File::from(
        fs::openat(
            directory,
            name,
            access | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|error| IoError::from(io::Error::from(error)))?,
    );
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(IoError::Io(io::ErrorKind::InvalidInput));
    }
    budget.charge(Resource::Steps, 1)?;
    require_private_file(&metadata)?;
    Ok((
        file,
        ObjectIdentity::from_metadata(&metadata),
        metadata.len(),
    ))
}

fn read_intent(
    file: &mut File,
    length: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, IoError> {
    check(budget, cancelled)?;
    let size = usize::try_from(length).map_err(|_| IoError::Io(io::ErrorKind::InvalidInput))?;
    budget.charge_allocation(length, 1)?;
    budget.charge(Resource::Steps, length)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| IoError::Io(io::ErrorKind::OutOfMemory))?;
    bytes.resize(size, 0);

    for (index, chunk) in bytes.chunks_mut(BLOCK_BYTES).enumerate() {
        budget.charge(Resource::Bytes, chunk.len() as u64)?;
        read_exact_at(file, (index * BLOCK_BYTES) as u64, chunk, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

fn require_child(
    directory: &HeldDirectory,
    name: &CStr,
    expected: ObjectIdentity,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    directory.verify_child(name, expected)
}

pub(super) fn reopen(
    target: &OfflineTarget,
    bundle_path: &Path,
    maximum_intent_bytes: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ExistingBundle, ReopenError> {
    let _span = tracing::debug_span!("io.journal.reopen_filesystem_pair").entered();
    target.require_entry(budget, cancelled)?;
    let location = Location::capture(bundle_path, budget, cancelled)?;
    reopen_location(target, location, maximum_intent_bytes, budget, cancelled)
}

fn reopen_location(
    target: &OfflineTarget,
    location: Location,
    maximum_intent_bytes: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ExistingBundle, ReopenError> {
    let (parent, name) = location.into_parts();
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, name.validation_work())
        .map_err(IoError::from)?;
    budget.charge(Resource::Steps, 1).map_err(IoError::from)?;
    let directory_file = File::from(
        fs::openat(
            &parent,
            name.as_c_str(),
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|error| IoError::from(io::Error::from(error)))?,
    );
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1).map_err(IoError::from)?;
    let directory_metadata = directory_file.metadata().map_err(IoError::from)?;
    budget.charge(Resource::Steps, 1).map_err(IoError::from)?;
    require_private_directory(&directory_metadata)?;
    let directory = HeldDirectory::adopt_observed(directory_file, &directory_metadata)?;
    let bundle_id = directory.identity();
    let (mut binding_file, binding_id, binding_length) =
        open_child(&directory, BINDING_ENTRY, false, budget, cancelled)?;
    if binding_length != RECORD_BYTES as u64 {
        return Err(ReopenError::BindingLength);
    }
    let mut binding_bytes = [0; RECORD_BYTES];
    budget
        .charge(Resource::Bytes, RECORD_BYTES as u64)
        .map_err(IoError::from)?;
    read_exact_at(&mut binding_file, 0, &mut binding_bytes, budget, cancelled)?;
    let data = binding::decode(&binding_bytes, budget, cancelled)?;
    if data.target != target.identity() || data.bundle != bundle_id || data.binding != binding_id {
        return Err(ReopenError::IdentityMismatch);
    }
    if data.intent_length > maximum_intent_bytes {
        return Err(ReopenError::IntentLimit);
    }

    let (mut intent_file, intent_id, intent_length) =
        open_child(&directory, INTENT_ENTRY, false, budget, cancelled)?;
    if intent_length > maximum_intent_bytes {
        return Err(ReopenError::IntentLimit);
    }
    if intent_length != data.intent_length {
        return Err(ReopenError::IntentMismatch);
    }
    let (journal_file, journal_id, _) =
        open_child(&directory, JOURNAL_ENTRY, true, budget, cancelled)?;
    // Reject aliases before attempting a second exclusive lock on a target role.
    if journal_id == target.identity() || journal_id == binding_id || journal_id == intent_id {
        return Err(ReopenError::IdentityMismatch);
    }
    let actual = PairObjects {
        target: target.identity(),
        bundle: bundle_id,
        binding: binding_id,
        intent: intent_id,
        journal: journal_id,
    };
    data.verify_objects(&actual)?;

    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 2).map_err(IoError::from)?;
    let journal = ExclusiveFile::from_owned(journal_file)?;
    let intent_bytes = read_intent(&mut intent_file, intent_length, budget, cancelled)?;
    match data.verify_intent(&intent_bytes, budget, cancelled) {
        Ok(()) => {}
        Err(BindingError::IntentLengthMismatch | BindingError::IntentDigestMismatch) => {
            return Err(ReopenError::IntentMismatch);
        }
        Err(BindingError::Io(cause)) => return Err(ReopenError::Io(cause)),
        Err(cause) => return Err(ReopenError::Binding(cause)),
    }

    // Bind each actual opened object to the held namespace before handoff.
    target.require_entry(budget, cancelled)?;
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, name.validation_work())
        .map_err(IoError::from)?;
    require_child(&parent, name.as_c_str(), bundle_id, budget, cancelled)?;
    require_child(&directory, BINDING_ENTRY, binding_id, budget, cancelled)?;
    require_child(&directory, INTENT_ENTRY, intent_id, budget, cancelled)?;
    require_child(&directory, JOURNAL_ENTRY, journal_id, budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(ExistingBundle {
        _parent: parent,
        name,
        _directory: directory,
        _binding_file: binding_file,
        _intent_file: intent_file,
        journal,
        binding: data,
        intent_bytes,
    })
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
