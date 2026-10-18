use super::{
    JournalError,
    format::{self, Record, State},
    storage::{self, BLOCK_BYTES, DurableStorage},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Durable outcome established by successful recovery of a retained journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryOutcome {
    /// The complete commit marker and exact new image agree.
    Committed,
    /// The original image is restored and synchronized; the journal is retained.
    RolledBack,
}

/// Verify all framing/digests and the paired target before any recovery write.
/// Pending or partial-marker transactions restore the original image; a complete
/// commit marker preserves only an exact new image. Recovery is restartable and
/// idempotent. It never deletes/reuses a journal. Failure requires explicit later
/// recovery with fresh limits; callers must not resume ordinary target access.
pub fn recover<D: DurableStorage, J: DurableStorage>(
    target: &mut D,
    journal: &mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoveryOutcome, JournalError> {
    let record = format::load(journal, budget, cancelled)?;
    verify_target(target, &record, record.state, budget, cancelled)?;

    if record.state == State::Pending {
        storage::write_image(target, 0, &record.before, budget, cancelled)?;
    }

    // Also re-establish barriers after an earlier sync returned an uncertain error.
    storage::sync(target, budget, cancelled)?;
    // Never overwrite a partial commit marker during rollback: another torn
    // overwrite could create an unrecognizable mixed marker. Keeping the
    // original journal makes interrupted and repeated rollback restartable.
    storage::sync(journal, budget, cancelled)?;

    Ok(if record.state == State::Committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    })
}

pub(super) fn verify_target<D: DurableStorage>(
    target: &mut D,
    record: &Record,
    state: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), JournalError> {
    storage::check(budget, cancelled)?;
    if target.storage_len()? != record.before.len() as u64 {
        return Err(JournalError::TargetMismatch);
    }

    let mut scratch = [0; BLOCK_BYTES];
    for (i, (before, after)) in record
        .before
        .chunks(BLOCK_BYTES)
        .zip(record.after.chunks(BLOCK_BYTES))
        .enumerate()
    {
        let current = &mut scratch[..before.len()];
        storage::read_at(target, (i * BLOCK_BYTES) as u64, current, budget, cancelled)?;
        budget.charge(Resource::Steps, current.len() as u64)?;
        let matches = match state {
            State::Committed => current == after,
            State::Original => current == before,
            State::Pending => current
                .iter()
                .zip(before)
                .zip(after)
                .all(|((now, old), new)| now == old || now == new),
        };
        if !matches {
            return Err(JournalError::TargetMismatch);
        }
    }

    storage::check(budget, cancelled)
}
