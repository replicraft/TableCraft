//! Fresh complete-row replay consumes the same explicitly inspected V1/V2 session.
use super::NativeCompleteInsertIntent;
use crate::{
    NativeImage,
    format::page::PAGE_BYTES,
    header::check,
    limits::MAX_IMAGE_BYTES,
    transaction::{NativeRecoveryQualificationError, qualified_recovery::equal_image},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedImage, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
};
use tablecraft_model::CatalogScope;

/// One original checked journal after complete native planning and image proof.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedNativeCompleteInsertRecovery<'a, J: DurableStorage> {
    checked: CheckedImage<'a, J>,
}

/// Verify source identity, independently derived placement and both full images.
/// Journal framing selects an inspector once; native planning must then agree
/// with that framing's geometry. A failed decoder or planner is never retried.
pub fn qualify_native_complete_insert_recovery<'a, J: DurableStorage>(
    checked: CheckedImage<'a, J>,
    intent: NativeCompleteInsertIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeCompleteInsertRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.complete_insert_recovery.qualify").entered();
    check(budget, cancelled)?;
    let (before, after, growing) = match &checked {
        CheckedImage::Fixed(session) => (session.before_bytes(), session.after_bytes(), false),
        CheckedImage::Resizable(session) => (session.before_bytes(), session.after_bytes(), true),
    };
    if after.len() < before.len()
        || after.len() as u64 > MAX_IMAGE_BYTES
        || !(after.len() - before.len()).is_multiple_of(PAGE_BYTES)
        || growing != (after.len() > before.len())
    {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    intent.verify_before(before, budget, cancelled)?;
    let image = NativeImage::capture(&mut Cursor::new(before), scope, budget, cancelled)?;
    let plan = image.plan_ordinary_complete_row_inserts(
        intent.table(),
        intent.rows(),
        budget,
        cancelled,
    )?;
    // No journal-provided length or stored placement selects native allocation.
    // The fresh planner derives its own first-fit EOF count from original rows.
    if growing != (plan.after_bytes().len() > plan.before_bytes().len()) {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    equal_image(plan.before_bytes(), before, budget, cancelled)?;
    equal_image(plan.after_bytes(), after, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedNativeCompleteInsertRecovery { checked })
}

impl<J: DurableStorage> QualifiedNativeCompleteInsertRecovery<'_, J> {
    /// Recover through this same retained journal and caller-owned target.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        match self.checked {
            CheckedImage::Fixed(session) => session.recover(target, budget, cancelled),
            CheckedImage::Resizable(session) => session.recover(target, budget, cancelled),
        }
    }

    /// Verify settled complete bytes and barriers without writes or resize.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        match self.checked {
            CheckedImage::Fixed(session) => session.confirm_completed(target, budget, cancelled),
            CheckedImage::Resizable(session) => {
                session.confirm_completed(target, budget, cancelled)
            }
        }
    }
}
