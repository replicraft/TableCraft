//! Exact native EOF reconstruction over the same immutable inspected V2 record.
use super::{
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateBody, NativeUpdateIntent,
    NativeUpdateProfile, qualified_recovery::equal_image,
};
use crate::{NativeImage, format::page::PAGE_BYTES, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// The same V2 journal after complete original native allocation replanning.
/// No native permission follows from the wire tag or generic journal integrity.
#[must_use = "recover explicitly or retain the journal for later diagnosis"]
pub struct QualifiedNativeAllocationRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

impl<J: DurableStorage> QualifiedNativeAllocationRecovery<'_, J> {
    /// Exact marker state loaded from this same immutable journal session.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Recover through the same inspected record and retained journal borrow.
    /// Pending restores/truncates BEFORE; committed requires exact AFTER.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.allocation_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm complete settled bytes and acknowledged barriers without writes
    /// or resize. The outcome describes current agreement, not past execution.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.allocation_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}

/// Rebuild one supported EOF allocation solely from journal BEFORE and original
/// requests, comparing both complete images before any target is supplied.
/// Profiles 1–6 and mismatched V2 lengths refuse without protocol fallback.
pub fn qualify_native_allocation_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeUpdateIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeAllocationRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.allocation_recovery.qualify").entered();
    check(budget, cancelled)?;
    let (profile, body) = intent.into_body();
    let (
        NativeUpdateProfile::Ace16OrdinaryEofAllocation,
        NativeUpdateBody::EofAllocation(original),
    ) = (profile, body)
    else {
        return Err(NativeIntentError::UnsupportedProfile.into());
    };
    if checked.before_bytes().len().checked_add(PAGE_BYTES) != Some(checked.after_bytes().len()) {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    original.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let groups = original.borrowed_groups(budget, cancelled)?;
    let plan = image.plan_ordinary_eof_updates(original.table(), &groups, budget, cancelled)?;
    if plan.before_bytes().len().checked_add(PAGE_BYTES) != Some(plan.after_bytes().len()) {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedNativeAllocationRecovery { checked })
}
