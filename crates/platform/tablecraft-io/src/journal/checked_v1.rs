//! Read-only V1 integrity session retaining one loaded record and journal borrow.
use super::{
    DurableStorage, JournalError, RecoveryOutcome,
    format::{self, Record, State},
    recovery, storage,
};
use tablecraft_core::limits::ResourceBudget;

/// State established by complete V1 framing, digest and marker inspection.
/// This supplies no target identity, native validation or recovery permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V1State {
    /// Complete before/after images with no marker or a valid marker prefix.
    Pending,
    /// Complete before/after images and an exact complete commit marker.
    Committed,
}

/// One integrity-checked V1 record retaining the same exclusive journal borrow.
/// Inspection and borrowed views never write or synchronize storage. Hosts still
/// own exclusive access, correct resource pairing and any native policy checks.
/// Dropping this session leaves the journal intact and does not recover a target.
///
/// The borrowed journal cannot be replaced while a session is retained:
///
/// ```compile_fail
/// use tablecraft_core::limits::ResourceBudget;
/// use tablecraft_io::journal::{DurableStorage, inspect_v1};
/// fn replace<J: DurableStorage>(journal: &mut J, replacement: J) {
///     let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
///     let checked = inspect_v1(journal, &mut budget, &|| false).unwrap();
///     *journal = replacement;
///     let _ = checked.before_bytes();
/// }
/// ```
pub struct CheckedV1<'a, J> {
    journal: &'a mut J,
    record: Record,
}

/// Read complete V1 framing, equal lengths, images, digest and marker state.
/// No target is supplied and no storage writes or syncs occur. The returned
/// session owns the paid immutable images and retains this same mutable journal
/// borrow; callers may validate policy against its views before consuming it.
/// Integrity is not authentication, target pairing or native write admission.
pub fn inspect_v1<'a, J: DurableStorage>(
    journal: &'a mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedV1<'a, J>, JournalError> {
    let _span = tracing::debug_span!("io.journal.inspect_v1").entered();
    let record = format::load(journal, budget, cancelled)?;
    Ok(CheckedV1 { journal, record })
}

impl<J> CheckedV1<'_, J> {
    /// Exact already inspected original image, borrowed without another copy.
    pub fn before_bytes(&self) -> &[u8] {
        &self.record.before
    }

    /// Exact already inspected replacement image, borrowed without another copy.
    pub fn after_bytes(&self) -> &[u8] {
        &self.record.after
    }

    /// Loaded marker state; this does not establish target agreement.
    pub fn state(&self) -> V1State {
        match self.record.state {
            State::Pending => V1State::Pending,
            State::Committed => V1State::Committed,
            State::Original => unreachable!("V1 loading never creates comparison-only state"),
        }
    }
}

impl<J: DurableStorage> CheckedV1<'_, J> {
    /// Consume the inspected record with the same journal, without reloading or
    /// accepting another journal. Target verification and all recovery barriers
    /// are identical to `recover`; all of its exclusive/pairing and host policy
    /// preconditions still apply. Failure retains recovery-required state.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        recovery::recover_loaded(target, self.journal, &self.record, budget, cancelled)
    }

    /// Verify a completed image without restoring or rewriting any byte. A
    /// committed record requires exact AFTER, a pending/partial marker exact
    /// BEFORE. Both storage barriers must acknowledge before returning. Outcome
    /// describes current complete bytes/barriers, not historical events.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("io.journal.confirm_completed").entered();
        let expected = match self.record.state {
            State::Committed => State::Committed,
            State::Pending => State::Original,
            State::Original => unreachable!("V1 loading never creates comparison-only state"),
        };
        recovery::verify_target(target, &self.record, expected, budget, cancelled)?;
        storage::sync(target, budget, cancelled)?;
        storage::sync(self.journal, budget, cancelled)?;
        Ok(if expected == State::Committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
    }
}

#[cfg(test)]
mod tests;
