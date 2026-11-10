//! Original TCNI17/18/19 replay over the same immutable checked V3 session.
use super::{
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
};
#[cfg(feature = "ace16-create")]
use super::{qualified_recovery::equal_image, update_rebuild};
#[cfg(feature = "ace16-create")]
use crate::NativeImage;
use crate::header::check;
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

/// The original V3 borrow after complete source/request/AFTER reconstruction.
#[must_use = "recover explicitly or retain the qualified V3 journal"]
pub struct QualifiedRetainedNativeUpdateRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl NativeUpdateIntent {
    /// Qualify only TCNI17/18/19 without replacing its checked journal owner.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedRetainedNativeUpdateRecovery<'a, J>, NativeRecoveryQualificationError>
    {
        let _span = tracing::debug_span!("native.retained_update_recovery.qualify").entered();
        check(budget, cancelled)?;
        if !matches!(
            self.profile(),
            NativeUpdateProfile::Ace16RetainedRows
                | NativeUpdateProfile::Ace16RetainedOpaqueRows
                | NativeUpdateProfile::Ace16RetainedCurrencyRows
        ) {
            return Err(NativeIntentError::UnsupportedProfile.into());
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
            let plan = update_rebuild::rebuild_verified_binding(image, &self, budget, cancelled)?;
            equal_image(
                plan.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
            check(budget, cancelled)?;
            Ok(QualifiedRetainedNativeUpdateRecovery { checked })
        }
        #[cfg(not(feature = "ace16-create"))]
        {
            let _ = (checked, scope);
            Err(crate::EditError::Unsupported(crate::WriteBlocker::RowLayout).into())
        }
    }
}

impl<J: DurableStorage> QualifiedRetainedNativeUpdateRecovery<'_, J> {
    /// State observed from this same checked V3 framing.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover through the exact qualified session and caller-owned target.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.retained_update_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm complete settled bytes/barriers without writing the target.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span =
            tracing::debug_span!("native.retained_update_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
