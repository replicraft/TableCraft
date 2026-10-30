use super::super::{
    JournalError, RecoveryOutcome,
    storage::{self, BLOCK_BYTES, DurableStorage, ResizableDurableStorage},
};
use super::format::{self, Record, State};
use tablecraft_core::limits::{Resource, ResourceBudget};

const LEGACY_MAGIC: &[u8; 8] = b"TCIMG001";

#[derive(Clone, Copy)]
pub(super) enum Verification {
    Original,
    Pending,
    Committed,
}

/// Recover an explicitly paired V1 or V2 journal. V1 keeps its fixed protocol.
/// Pending V2 records restore their complete before image and truncate last;
/// committed V2 records require exact after bytes/length and never rewrite them.
///
/// Lengths between the two images and real per-byte origins are checked before
/// pending writes. Missing tails are never synthesized as expected zero bytes.
/// Journals remain intact; every failure requires diagnosis or explicit recovery
/// before normal access. Exclusive pairing and durable namespace remain host-owned.
pub fn recover_resizable<D: ResizableDurableStorage, J: DurableStorage>(
    target: &mut D,
    journal: &mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoveryOutcome, JournalError> {
    let _span = tracing::debug_span!("io.journal.recover_resizable").entered();
    storage::check(budget, cancelled)?;
    if journal.storage_len()? < format::MAGIC.len() as u64 {
        return Err(JournalError::InvalidJournal);
    }
    let mut magic = [0; format::MAGIC.len()];
    storage::read_at(journal, 0, &mut magic, budget, cancelled)?;
    if &magic == LEGACY_MAGIC {
        return super::super::recover(target, journal, budget, cancelled);
    }
    if &magic != format::MAGIC {
        return Err(JournalError::InvalidJournal);
    }

    let record = format::load(journal, budget, cancelled)?;
    let verification = match record.state {
        State::Pending => Verification::Pending,
        State::Committed => Verification::Committed,
    };
    verify_target(target, &record, verification, budget, cancelled)?;

    if record.state == State::Pending {
        storage::write_image(target, 0, &record.before, budget, cancelled)?;
        storage::resize(target, record.before.len() as u64, budget, cancelled)?;
    }
    storage::sync(target, budget, cancelled)?;
    storage::sync(journal, budget, cancelled)?;

    Ok(match record.state {
        State::Pending => RecoveryOutcome::RolledBack,
        State::Committed => RecoveryOutcome::Committed,
    })
}

pub(super) fn verify_target<D: DurableStorage>(
    target: &mut D,
    record: &Record,
    state: Verification,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    storage::check(budget, cancelled)?;
    let length = target.storage_len()?;
    let before = record.before.len() as u64;
    let after = record.after.len() as u64;
    let valid_length = match state {
        Verification::Original => length == before,
        Verification::Committed => length == after,
        Verification::Pending => (before.min(after)..=before.max(after)).contains(&length),
    };
    if !valid_length {
        return Err(JournalError::TargetMismatch);
    }

    let length = usize::try_from(length).map_err(|_| JournalError::TargetMismatch)?;
    let mut scratch = [0; BLOCK_BYTES];
    for start in (0..length).step_by(BLOCK_BYTES) {
        let count = (length - start).min(BLOCK_BYTES);
        let current = &mut scratch[..count];
        storage::read_at(target, start as u64, current, budget, cancelled)?;
        budget.charge(Resource::Steps, count as u64)?;
        for (position, now) in current.iter().enumerate() {
            let offset = start + position;
            let matches = match state {
                Verification::Original => record.before.get(offset) == Some(now),
                Verification::Committed => record.after.get(offset) == Some(now),
                Verification::Pending => {
                    record.before.get(offset) == Some(now) || record.after.get(offset) == Some(now)
                }
            };
            if !matches {
                return Err(JournalError::TargetMismatch);
            }
        }
    }

    storage::check(budget, cancelled)
}
