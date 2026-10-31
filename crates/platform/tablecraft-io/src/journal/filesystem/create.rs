//! Establish fresh paired objects and barriers without touching target bytes.
use super::{
    BINDING_ENTRY, INTENT_ENTRY, JOURNAL_ENTRY, OfflineTarget, PairError,
    binding::{self, BindingData, BindingError, DIGEST_BYTES, PairObjects},
    pair::{NONCE_BYTES, retirement_name},
    reopen::ExistingBundle,
};
use crate::{
    ExclusiveFile, IoError,
    journal::storage::BLOCK_BYTES,
    namespace::{
        HeldDirectory, Location, ObjectIdentity, RelativeName, require_private_directory,
        require_private_file,
    },
    progress::check,
    write_exact_at,
};
use rustix::fs::{self, Mode, OFlags};
use std::{ffi::CStr, fs::File, io, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_NONCE_ATTEMPTS: usize = 16;
const RETAINED_SETUP_SLOTS: usize = 1;

/// Last acknowledged setup phase. Attempted directory creation is retained
/// separately because a failed callback must not be treated as ownership proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CreatePhase {
    Captured,
    DirectoryAttempted,
    DirectoryCreated,
    DirectoryOpened,
    BindingCreated,
    IntentCreated,
    JournalCreated,
    RecordsWritten,
    FilesSynced,
    DirectorySynced,
    ParentSynced,
}

/// Handles remain paired with the captured namespace after a partial setup.
/// Drop only closes handles; it never removes, replaces or retries an entry.
pub(super) struct IncompleteBundle {
    pub parent: HeldDirectory,
    pub name: RelativeName,
    pub directory: Option<HeldDirectory>,
    pub directory_file: Option<File>,
    pub binding_file: Option<File>,
    pub intent_file: Option<File>,
    pub journal: Option<ExclusiveFile>,
    pub journal_file: Option<File>,
    pub intent_bytes: Vec<u8>,
    pub phase: CreatePhase,
}

pub(super) struct CreateFailure {
    pub cause: PairError,
    pub incomplete: Vec<IncompleteBundle>,
}

impl CreateFailure {
    fn before(cause: impl Into<PairError>) -> Self {
        Self {
            cause: cause.into(),
            incomplete: Vec::new(),
        }
    }
}

/// The pair caller prepays terminal work before entering this function. Target
/// baseline and every new file/directory barrier finish before native prepare.
pub(super) fn create(
    target: &mut OfflineTarget,
    bundle: &Path,
    intent: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ExistingBundle, CreateFailure> {
    let _span = tracing::debug_span!("io.journal.create_pair_objects").entered();
    // Reserve the one possible partial owner before any durability or namespace
    // callback. A late failure moves it into this slot without allocating.
    check(budget, cancelled).map_err(CreateFailure::before)?;
    budget
        .charge_allocation(
            RETAINED_SETUP_SLOTS as u64,
            size_of::<IncompleteBundle>() as u64,
        )
        .map_err(IoError::from)
        .map_err(CreateFailure::before)?;
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(RETAINED_SETUP_SLOTS)
        .map_err(|_| CreateFailure::before(IoError::Io(io::ErrorKind::OutOfMemory)))?;

    target
        .sync_baseline(budget, cancelled)
        .map_err(CreateFailure::before)?;
    let location = Location::capture(bundle, budget, cancelled).map_err(CreateFailure::before)?;
    let (parent, name) = location.into_parts();
    let nonce = fresh_nonce(&name, budget, cancelled).map_err(CreateFailure::before)?;
    let intent_digest = binding::digest_intent(&intent, budget, cancelled)
        .map_err(binding_error)
        .map_err(CreateFailure::before)?;

    // Discover unsupported captured-parent barriers before creating an entry.
    step(budget, cancelled).map_err(CreateFailure::before)?;
    parent.sync().map_err(CreateFailure::before)?;
    let mut incomplete = IncompleteBundle {
        parent,
        name,
        directory: None,
        directory_file: None,
        binding_file: None,
        intent_file: None,
        journal: None,
        journal_file: None,
        intent_bytes: intent,
        phase: CreatePhase::Captured,
    };
    let data = match populate(
        target,
        &mut incomplete,
        nonce,
        intent_digest,
        budget,
        cancelled,
    ) {
        Ok(data) => data,
        Err(cause) => {
            retained.push(incomplete);
            return Err(CreateFailure {
                cause,
                incomplete: retained,
            });
        }
    };

    Ok(ExistingBundle {
        _parent: incomplete.parent,
        name: incomplete.name,
        _directory: incomplete.directory.expect("complete setup owns directory"),
        _binding_file: incomplete
            .binding_file
            .expect("complete setup owns binding"),
        _intent_file: incomplete.intent_file.expect("complete setup owns intent"),
        journal: incomplete.journal.expect("complete setup owns journal"),
        binding: data,
        intent_bytes: incomplete.intent_bytes,
    })
}

fn populate(
    target: &OfflineTarget,
    state: &mut IncompleteBundle,
    nonce: [u8; NONCE_BYTES],
    intent_digest: [u8; DIGEST_BYTES],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BindingData, PairError> {
    step(budget, cancelled)?;
    budget
        .charge(Resource::Steps, state.name.validation_work())
        .map_err(IoError::from)?;
    state.phase = CreatePhase::DirectoryAttempted;
    fs::mkdirat(&state.parent, state.name.as_c_str(), Mode::RWXU)
        .map_err(io::Error::from)
        .map_err(IoError::from)?;
    state.phase = CreatePhase::DirectoryCreated;

    step(budget, cancelled)?;
    budget
        .charge(Resource::Steps, state.name.validation_work())
        .map_err(IoError::from)?;
    state.directory_file = Some(File::from(
        fs::openat(
            &state.parent,
            state.name.as_c_str(),
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)
        .map_err(IoError::from)?,
    ));
    step(budget, cancelled)?;
    let metadata = state
        .directory_file
        .as_ref()
        .expect("created directory handle")
        .metadata()
        .map_err(IoError::from)?;
    step(budget, cancelled)?;
    require_private_directory(&metadata)?;
    state.directory = Some(HeldDirectory::adopt_observed(
        state
            .directory_file
            .take()
            .expect("created directory handle"),
        &metadata,
    )?);
    state.phase = CreatePhase::DirectoryOpened;

    let directory = state.directory.as_ref().expect("opened directory");
    let binding_id = create_file(
        &mut state.binding_file,
        directory,
        BINDING_ENTRY,
        budget,
        cancelled,
    )?;
    state.phase = CreatePhase::BindingCreated;
    let intent_id = create_file(
        &mut state.intent_file,
        directory,
        INTENT_ENTRY,
        budget,
        cancelled,
    )?;
    state.phase = CreatePhase::IntentCreated;
    let journal_id = create_file(
        &mut state.journal_file,
        directory,
        JOURNAL_ENTRY,
        budget,
        cancelled,
    )?;
    step(budget, cancelled)?;
    budget.charge(Resource::Steps, 2).map_err(IoError::from)?;
    state.journal = Some(ExclusiveFile::from_owned(
        state.journal_file.take().expect("created journal handle"),
    )?);
    state.phase = CreatePhase::JournalCreated;

    let objects = PairObjects {
        target: target.identity(),
        bundle: directory.identity(),
        binding: binding_id,
        intent: intent_id,
        journal: journal_id,
    };
    let data = BindingData {
        nonce,
        target: objects.target,
        bundle: objects.bundle,
        binding: objects.binding,
        intent: objects.intent,
        journal: objects.journal,
        intent_length: state.intent_bytes.len() as u64,
        intent_digest,
    };
    let bytes = data.encode(budget, cancelled).map_err(binding_error)?;
    write_file(
        state.intent_file.as_mut().expect("created intent"),
        &state.intent_bytes,
        budget,
        cancelled,
    )?;
    write_file(
        state.binding_file.as_mut().expect("created binding"),
        &bytes,
        budget,
        cancelled,
    )?;
    state.phase = CreatePhase::RecordsWritten;

    for file in [&state.intent_file, &state.binding_file] {
        step(budget, cancelled)?;
        file.as_ref()
            .expect("created record")
            .sync_all()
            .map_err(IoError::from)?;
    }
    step(budget, cancelled)?;
    state
        .journal
        .as_mut()
        .expect("created journal")
        .sync_all()
        .map_err(IoError::from)?;
    state.phase = CreatePhase::FilesSynced;
    step(budget, cancelled)?;
    directory.sync()?;
    state.phase = CreatePhase::DirectorySynced;
    step(budget, cancelled)?;
    state.parent.sync()?;
    state.phase = CreatePhase::ParentSynced;

    target.require_entry(budget, cancelled)?;
    step(budget, cancelled)?;
    budget
        .charge(Resource::Steps, state.name.validation_work())
        .map_err(IoError::from)?;
    state
        .parent
        .verify_child(state.name.as_c_str(), objects.bundle)?;
    for (name, identity) in [
        (BINDING_ENTRY, binding_id),
        (INTENT_ENTRY, intent_id),
        (JOURNAL_ENTRY, journal_id),
    ] {
        step(budget, cancelled)?;
        directory.verify_child(name, identity)?;
    }
    check(budget, cancelled)?;
    Ok(data)
}

fn step(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), IoError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn binding_error(error: BindingError) -> PairError {
    match error {
        BindingError::Io(error) => PairError::Io(error),
        error => PairError::Binding(error),
    }
}

fn fresh_nonce(
    name: &RelativeName,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; NONCE_BYTES], PairError> {
    for _ in 0..MAX_NONCE_ATTEMPTS {
        step(budget, cancelled)?;
        budget
            .charge(Resource::Steps, NONCE_BYTES as u64)
            .map_err(IoError::from)?;
        let mut nonce = [0; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| IoError::Io(io::ErrorKind::Other))?;
        let reserved = retirement_name(&nonce);
        budget
            .charge(
                Resource::Steps,
                reserved.len() as u64 + name.validation_work(),
            )
            .map_err(IoError::from)?;
        if name.as_bytes() != reserved.as_slice() {
            return Ok(nonce);
        }
    }
    Err(PairError::ReservedName)
}

fn create_file(
    slot: &mut Option<File>,
    directory: &HeldDirectory,
    name: &CStr,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ObjectIdentity, IoError> {
    step(budget, cancelled)?;
    *slot = Some(File::from(
        fs::openat(
            directory,
            name,
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(io::Error::from)?,
    ));
    step(budget, cancelled)?;
    let metadata = slot.as_ref().expect("created child handle").metadata()?;
    step(budget, cancelled)?;
    require_private_file(&metadata)?;
    Ok(ObjectIdentity::from_metadata(&metadata))
}

fn write_file(
    file: &mut File,
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    for (index, chunk) in bytes.chunks(BLOCK_BYTES).enumerate() {
        budget.charge(Resource::Bytes, chunk.len() as u64)?;
        write_exact_at(file, (index * BLOCK_BYTES) as u64, chunk, budget, cancelled)?;
    }
    check(budget, cancelled)
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
