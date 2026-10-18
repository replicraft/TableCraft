use super::{
    CommitError, JournalError, RecoveryOutcome,
    format::{self, HEADER_BYTES, MAX_IMAGE_BYTES, Record, State},
    recover,
    recovery::verify_target,
    storage::{self, DurableStorage},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Prepared fixed-length image replacement. The journal is synced before this
/// value is returned; the target is still unchanged. No payload Debug or clone.
/// Dropping it leaves the prepared journal intact; recover before normal access.
#[must_use = "commit or explicitly recover the retained prepared journal"]
pub struct PreparedImage<'a, D, J> {
    target: &'a mut D,
    journal: &'a mut J,
    record: Record,
}

/// Prepare a bounded image journal without changing the target. The journal must
/// be empty and its namespace already durably established by the host. Equal,
/// nonempty image lengths up to 64 MiB are supported; no resizing or page format
/// is inferred. Failure can leave an incomplete journal but never changes target
/// bytes. Hosts must retain and diagnose incomplete journals, not overwrite them.
pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
    target: &'a mut D,
    journal: &'a mut J,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedImage<'a, D, J>, JournalError> {
    prepare_inner(target, journal, None, after, budget, cancelled)
}

/// Prepare only if the target exactly matches a previously validated image.
/// Mismatch fails before any journal or target write. All ordinary prepare
/// durability, exclusive-access, pairing and recovery preconditions still apply.
pub fn prepare_expected<'a, D: DurableStorage, J: DurableStorage>(
    target: &'a mut D,
    journal: &'a mut J,
    expected: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedImage<'a, D, J>, JournalError> {
    if expected.len() != after.len() {
        return Err(JournalError::TargetMismatch);
    }
    prepare_inner(target, journal, Some(expected), after, budget, cancelled)
}

fn prepare_inner<'a, D: DurableStorage, J: DurableStorage>(
    target: &'a mut D,
    journal: &'a mut J,
    expected: Option<&[u8]>,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedImage<'a, D, J>, JournalError> {
    storage::check(budget, cancelled)?;
    if journal.storage_len()? != 0 {
        return Err(JournalError::JournalNotEmpty);
    }
    let length = target.storage_len()?;
    if length == 0 || length > MAX_IMAGE_BYTES || length != after.len() as u64 {
        return Err(JournalError::ImageSize);
    }

    let before = storage::read_image(target, 0, after.len(), budget, cancelled)?;
    if let Some(expected) = expected {
        for (actual, expected) in before
            .chunks(storage::BLOCK_BYTES)
            .zip(expected.chunks(storage::BLOCK_BYTES))
        {
            storage::check(budget, cancelled)?;
            budget.charge(Resource::Steps, actual.len() as u64)?;
            if actual != expected {
                return Err(JournalError::TargetMismatch);
            }
        }
    }
    budget.charge_allocation(after.len() as u64, 1)?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(after.len())
        .map_err(|_| JournalError::Allocation)?;
    owned.extend_from_slice(after);

    let digest = format::digest(&before, &owned, budget, cancelled)?;
    let record = Record {
        before,
        after: owned,
        digest,
        state: State::Pending,
    };

    storage::write_at(journal, 0, &record.header(), budget, cancelled)?;
    storage::write_image(
        journal,
        HEADER_BYTES as u64,
        &record.before,
        budget,
        cancelled,
    )?;
    storage::write_image(
        journal,
        HEADER_BYTES as u64 + length,
        &record.after,
        budget,
        cancelled,
    )?;
    storage::sync(journal, budget, cancelled)?;

    Ok(PreparedImage {
        target,
        journal,
        record,
    })
}

impl<D: DurableStorage, J: DurableStorage> PreparedImage<'_, D, J> {
    /// Recheck the old image, write/sync the new target, then write/sync a commit
    /// marker. Any error requires recovery and does not imply a rollback. A
    /// successful commit retains its journal; host-controlled cleanup follows.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        self.commit_inner(budget, cancelled).map_err(CommitError)
    }

    fn commit_inner(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), JournalError> {
        verify_target(
            self.target,
            &self.record,
            State::Original,
            budget,
            cancelled,
        )?;

        storage::write_image(self.target, 0, &self.record.after, budget, cancelled)?;
        storage::sync(self.target, budget, cancelled)?;

        storage::write_at(
            self.journal,
            self.record.body_end(),
            &self.record.marker(),
            budget,
            cancelled,
        )?;
        storage::sync(self.journal, budget, cancelled)
    }

    /// Restore the original image through the same checked recovery path.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let Self {
            target,
            journal,
            record,
        } = self;
        drop(record);
        recover(target, journal, budget, cancelled)
    }
}
