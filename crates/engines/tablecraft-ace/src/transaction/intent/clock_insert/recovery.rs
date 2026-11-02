//! Fresh original-presence/default INSERT replay before consuming the same checked V1 session.
use super::NativeClockInsertIntent;
use crate::transaction::{NativeRecoveryQualificationError, qualified_recovery::equal_image};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The original checked journal after complete source-bound INSERT replanning.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedNativeClockInsertRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Reconstruct complete native admission, original witnesses and both images.
/// Neither a decoded TCNA0005 record nor persisted AFTER bytes grants authority.
pub fn qualify_native_clock_insert_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeClockInsertIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeClockInsertRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.clock_insert_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = image.plan_ordinary_clock_row_inserts(
        intent.table(),
        intent.rows(),
        intent.reading(),
        budget,
        cancelled,
    )?;
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedNativeClockInsertRecovery { checked })
}

impl<J: DurableStorage> QualifiedNativeClockInsertRecovery<'_, J> {
    /// Framing state of this same immutable checked journal session.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Recover through the retained checked session without recapture or cleanup.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.clock_insert_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify settled current bytes and barriers without target writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span =
            tracing::debug_span!("native.clock_insert_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
