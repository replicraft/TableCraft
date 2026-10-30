use super::super::{
    CommitError, JournalError, RecoveryOutcome,
    format::MAX_IMAGE_BYTES,
    storage::{self, DurableStorage, ResizableDurableStorage},
};
use super::{
    format::{self, HEADER_BYTES, Record, State},
    images,
    recovery::{Verification, recover_resizable, verify_target},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// A synchronized V2 journal retaining independently sized original and
/// replacement images. Target writes require the explicit resizable capability.
/// Dropping retains the journal and requires recovery before ordinary access.
#[must_use = "commit or explicitly recover the retained image journal"]
pub struct PreparedResizableImage<'a, D, J> {
    target: &'a mut D,
    journal: &'a mut J,
    record: Record,
}

/// Prepare a V2 journal only when the target exactly matches the caller's before
/// image. Each image may independently be empty or at most 64 MiB. Failure never
/// changes the target, but may leave an incomplete journal requiring diagnosis.
///
/// The host must exclusively own distinct, correctly paired resources and make
/// journal namespace creation durable beforehand. No file creation, pairing,
/// database authorization, cleanup or engineering-preview promotion is supplied.
///
/// ```compile_fail
/// use tablecraft_core::limits::ResourceBudget;
/// use tablecraft_io::journal::{DurableStorage, prepare_resizable_expected};
///
/// fn fixed_only<D: DurableStorage, J: DurableStorage>(
///     target: &mut D,
///     journal: &mut J,
///     budget: &mut ResourceBudget,
/// ) {
///     let _ = prepare_resizable_expected(target, journal, b"old", b"longer", budget, &|| false);
/// }
/// ```
pub fn prepare_resizable_expected<'a, D: ResizableDurableStorage, J: DurableStorage>(
    target: &'a mut D,
    journal: &'a mut J,
    expected: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
    let _span = tracing::debug_span!("io.journal.prepare_resizable").entered();
    storage::check(budget, cancelled)?;
    if journal.storage_len()? != 0 {
        return Err(JournalError::JournalNotEmpty);
    }
    let length = target.storage_len()?;
    if length > MAX_IMAGE_BYTES || after.len() as u64 > MAX_IMAGE_BYTES {
        return Err(JournalError::ImageSize);
    }
    if length != expected.len() as u64 {
        return Err(JournalError::TargetMismatch);
    }

    let size = usize::try_from(length).map_err(|_| JournalError::ImageSize)?;
    let before = images::read(target, 0, size, budget, cancelled)?;
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
    let after = images::copy(after, budget, cancelled)?;
    let digest = format::digest(&before, &after, budget, cancelled)?;
    let record = Record {
        before,
        after,
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

    Ok(PreparedResizableImage {
        target,
        journal,
        record,
    })
}

impl<D: ResizableDurableStorage, J: DurableStorage> PreparedResizableImage<'_, D, J> {
    /// Recheck exact original bytes/length, write the complete replacement,
    /// establish its final length and synchronize it before the commit marker.
    /// Every error requires recovery; errors do not imply rollback.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        let _span = tracing::debug_span!("io.journal.commit_resizable").entered();
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
            Verification::Original,
            budget,
            cancelled,
        )?;

        storage::write_image(self.target, 0, &self.record.after, budget, cancelled)?;
        storage::resize(
            self.target,
            self.record.after.len() as u64,
            budget,
            cancelled,
        )?;
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

    /// Restore the before image through checked pending recovery. The journal
    /// remains retained and normal access must wait for a successful outcome.
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
        recover_resizable(target, journal, budget, cancelled)
    }
}
