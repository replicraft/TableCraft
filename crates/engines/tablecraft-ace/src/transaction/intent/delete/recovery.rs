//! Fresh raw DELETE replay before consuming the same checked V1 session.
use super::NativeDeleteIntent;
use crate::transaction::{NativeRecoveryQualificationError, qualified_recovery::equal_image};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The original checked journal after complete source-bound DELETE replanning.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedNativeDeleteRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Reconstruct complete native admission, original witnesses and both images.
/// Neither a decoded TCND record nor persisted AFTER bytes grants authority.
pub fn qualify_native_delete_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeDeleteIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeDeleteRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.delete_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = intent.plan(image, budget, cancelled)?;
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedNativeDeleteRecovery { checked })
}

impl<J: DurableStorage> QualifiedNativeDeleteRecovery<'_, J> {
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
        let _span = tracing::debug_span!("native.delete_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify settled current bytes and barriers without target writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.delete_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
