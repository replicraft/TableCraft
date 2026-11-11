//! TCFD0003/4/5 replays only after requalifying the same checked V3 journal.
use super::super::{FieldDropIntentError, NativeFieldDropIntent};
use crate::header::check;
#[cfg(feature = "ace16-create")]
use crate::{NativeImage, transaction::qualified_recovery::equal_image};
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

/// The same immutable V3 session, qualified by the original TCFD0003/4/5 source proof.
#[must_use = "recover explicitly or retain the qualified V3 journal"]
pub struct QualifiedRetainedFieldDropRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl NativeFieldDropIntent {
    /// Qualify only retained TCFD0003/4/5 against original BEFORE and complete AFTER.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedRetainedFieldDropRecovery<'a, J>, FieldDropIntentError> {
        let _span = tracing::debug_span!("native.retained_field_drop_recovery.qualify").entered();
        check(budget, cancelled)?;
        if !self.is_retained() && !self.is_retained_typed() && !self.is_retained_non_tail() {
            return Err(FieldDropIntentError::InvalidRecord);
        }
        #[cfg(feature = "ace16-create")]
        {
            self.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture_retained(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let plan = self.reconstruct(image, budget, cancelled)?;
            equal_image(
                plan.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
            check(budget, cancelled)?;
            Ok(QualifiedRetainedFieldDropRecovery { checked })
        }
        #[cfg(not(feature = "ace16-create"))]
        {
            let _ = (checked, scope);
            Err(FieldDropIntentError::UnsupportedSource)
        }
    }
}

impl<J: DurableStorage> QualifiedRetainedFieldDropRecovery<'_, J> {
    /// Exact V3 framing state, not a substitute for source qualification.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover through this same checked owner with explicit resizable storage.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.retained_field_drop_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify settled bytes and barriers without replaying or writing the target.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span =
            tracing::debug_span!("native.retained_field_drop_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
