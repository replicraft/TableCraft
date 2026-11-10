//! One inspected version-typed resizable record retaining the original journal borrow.
use super::{
    format::{self, Protocol, Record, State},
    recovery::{self, Verification},
};
use crate::journal::{
    DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, storage,
};
use tablecraft_core::limits::ResourceBudget;

/// State established by complete versioned framing, image digest and marker inspection.
/// Integrity alone supplies no native validation, target identity or permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizableState {
    /// Complete independently sized images and no marker or a valid prefix.
    Pending,
    /// Complete images and an exact full versioned commit marker.
    Committed,
}

/// Immutable paid version-bound images plus the same exclusive journal borrow. Inspection
/// never writes or synchronizes either storage. Hosts still own native policy,
/// correct exclusive pairing and durable namespace handling. Dropping retains
/// the journal and never implicitly recovers a target.
///
/// The original journal cannot be replaced while this token is retained:
///
/// ```compile_fail
/// use tablecraft_core::limits::ResourceBudget;
/// use tablecraft_io::journal::{DurableStorage, inspect_v2};
/// fn replace<J: DurableStorage>(journal: &mut J, replacement: J) {
///     let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
///     let checked = inspect_v2(journal, &mut budget, &|| false).unwrap();
///     *journal = replacement;
///     let _ = checked.before_bytes();
/// }
/// ```
pub struct CheckedResizableImage<'a, J, const VERSION: u8> {
    journal: &'a mut J,
    record: Record,
}

/// Closed V2 inspected-image capability.
pub type CheckedV2<'a, J> = CheckedResizableImage<'a, J, 2>;
/// Closed V3 inspected-image capability; V2 records cannot construct it.
pub type CheckedV3<'a, J> = CheckedResizableImage<'a, J, 3>;
/// Marker state of an inspected V2 journal.
pub type V2State = ResizableState;
/// Marker state of an inspected V3 journal.
pub type V3State = ResizableState;

/// Load V2 only using the existing complete grammar/digest/marker validator.
/// This retains one immutable record without target effects, writes or syncs.
/// V1 and unknown formats refuse; no fallback selects a different validator.
pub fn inspect_v2<'a, J: DurableStorage>(
    journal: &'a mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedV2<'a, J>, JournalError> {
    let _span = tracing::debug_span!("io.journal.inspect_v2").entered();
    let record = format::load(journal, Protocol::V2, budget, cancelled)?;
    Ok(CheckedResizableImage { journal, record })
}

/// Inspect only V3 framing, length-bound digest and marker. V1/V2 refuse.
/// This retains the original exclusive journal borrow without storage effects.
pub fn inspect_v3<'a, J: DurableStorage>(
    journal: &'a mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedV3<'a, J>, JournalError> {
    let _span = tracing::debug_span!("io.journal.inspect_v3").entered();
    let record = format::load(journal, Protocol::V3, budget, cancelled)?;
    Ok(CheckedResizableImage { journal, record })
}

impl<J, const VERSION: u8> CheckedResizableImage<'_, J, VERSION> {
    /// Exact already inspected original image, with no additional copy.
    pub fn before_bytes(&self) -> &[u8] {
        &self.record.before
    }

    /// Exact already inspected replacement image, independently sized.
    pub fn after_bytes(&self) -> &[u8] {
        &self.record.after
    }

    /// Loaded marker state, without any assertion about current target bytes.
    pub fn state(&self) -> ResizableState {
        match self.record.state {
            State::Pending => ResizableState::Pending,
            State::Committed => ResizableState::Committed,
        }
    }
}

impl<J: DurableStorage, const VERSION: u8> CheckedResizableImage<'_, J, VERSION> {
    /// Consume the same inspected journal after caller policy qualification.
    /// Verify mixed-byte origins/length before recovery effects; pending records
    /// restore BEFORE then truncate, committed records verify AFTER unchanged.
    /// Failures retain recovery-required state and never retry another protocol.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("io.journal.checked_resizable.recover").entered();
        recovery::recover_loaded(target, self.journal, &self.record, budget, cancelled)
    }

    /// Confirm complete settled bytes and both acknowledged barriers, without
    /// any write or resize. Pending requires exact BEFORE, committed exact AFTER;
    /// mixed intermediate images refuse. Returned outcome describes current
    /// image agreement, not proof of a historical commit or rollback.
    /// A resizable target capability is deliberately unnecessary.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span =
            tracing::debug_span!("io.journal.checked_resizable.confirm_completed").entered();
        let expected = match self.record.state {
            State::Pending => Verification::Original,
            State::Committed => Verification::Committed,
        };
        recovery::verify_target(target, &self.record, expected, budget, cancelled)?;
        storage::sync(target, budget, cancelled)?;
        storage::sync(self.journal, budget, cancelled)?;
        Ok(match self.record.state {
            State::Pending => RecoveryOutcome::RolledBack,
            State::Committed => RecoveryOutcome::Committed,
        })
    }
}

#[cfg(test)]
mod tests;
