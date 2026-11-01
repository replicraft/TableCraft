//! Fresh typed INSERT replay before consuming the same checked V2 session.
use super::NativeEofInsertIntent;
use crate::transaction::{NativeRecoveryQualificationError, qualified_recovery::equal_image};
use crate::{NativeImage, format::page::PAGE_BYTES, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// The original checked journal after complete source-bound INSERT replanning.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedNativeEofInsertRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Reconstruct complete native admission, original witnesses and both images.
/// Neither a decoded TCNA record nor persisted AFTER bytes grants authority.
pub fn qualify_native_eof_insert_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeEofInsertIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeEofInsertRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.eof_insert_recovery.qualify").entered();
    check(budget, cancelled)?;
    if checked.before_bytes().len().checked_add(PAGE_BYTES) != Some(checked.after_bytes().len()) {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan =
        image.plan_ordinary_eof_row_inserts(intent.table(), intent.rows(), budget, cancelled)?;
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedNativeEofInsertRecovery { checked })
}

impl<J: DurableStorage> QualifiedNativeEofInsertRecovery<'_, J> {
    /// Framing state of this same immutable checked journal session.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Recover through the retained checked session without recapture or cleanup.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.eof_insert_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify settled current bytes and barriers without target writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.eof_insert_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
