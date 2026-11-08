use super::{AddTableIntentError, NativeAddTableIntent, codec::Profile};
use crate::{
    NativeImage, NativeRecoveryQualificationError, header::check,
    transaction::qualified_recovery::equal_image,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// The same immutable V2 journal after source-bound add-table reconstruction.
#[must_use = "recover explicitly or retain the qualified add-table journal"]
pub struct QualifiedAddTableRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Requalify the actual populated BEFORE and rebuild the original declared AFTER before recovery.
/// The persisted intent and replacement image are data, never mutation authority.
pub fn qualify_native_add_table_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeAddTableIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.add_table_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = if intent.profile == Profile::Mixed {
        let plan = image.plan_add_table(intent.definition, budget, cancelled)?;
        if !plan.requires_mixed_intent() {
            return Err(AddTableIntentError::UnsupportedProfile.into());
        }
        plan
    } else {
        image.plan_add_table_legacy(intent.definition, budget, cancelled)?
    };
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedAddTableRecovery { checked })
}

impl<J: DurableStorage> QualifiedAddTableRecovery<'_, J> {
    /// Observed framing state of this exact checked session.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Recover through the same checked session, without recapture or cleanup.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.add_table_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify a settled current image without repairing or replaying anything.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.add_table_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
