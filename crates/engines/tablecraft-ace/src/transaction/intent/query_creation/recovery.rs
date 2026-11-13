//! Fresh complete reconstruction qualifies the same checked V3 owner.
use super::{NativeQueryCreationIntent as Intent, QueryCreationIntentError as Error};
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

/// The same V3 session, qualified by native source and complete AFTER reconstruction.
#[must_use = "recover explicitly or retain the checked native query creation journal"]
pub struct QualifiedNativeQueryCreationRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl Intent {
    /// Verify complete BEFORE, recapture its retained source and reconstruct the
    /// semantic definition, allocated ID and complete AFTER before target effects.
    /// No clock, SQL execution, patched AFTER or other action decoder is used.
    ///
    /// # Errors
    /// Refuses binding/identity/source/image mismatch, allocation, cancellation
    /// and caller limits. Without native creation this returns UnsupportedSource.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedNativeQueryCreationRecovery<'a, J>, Error> {
        let _span = tracing::debug_span!("native.query_creation.recovery_qualify").entered();
        check(budget, cancelled)?;
        #[cfg(feature = "ace16-create")]
        {
            self.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture_retained(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let (effect, _original) = self.reconstruct(image, budget, cancelled)?;
            equal_image(
                effect.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(
                effect.after_bytes(),
                checked.after_bytes(),
                budget,
                cancelled,
            )?;
            check(budget, cancelled)?;
            Ok(QualifiedNativeQueryCreationRecovery { checked })
        }
        #[cfg(not(feature = "ace16-create"))]
        {
            let _ = (checked, scope);
            Err(Error::UnsupportedSource)
        }
    }
}

impl<J: DurableStorage> QualifiedNativeQueryCreationRecovery<'_, J> {
    /// Exact framing state captured by the same retained checked owner.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover through this same qualified owner and explicit resizable target.
    ///
    /// # Errors
    /// Propagates target/journal I/O, barriers, source mismatch and caller limits.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm a settled complete image without replay or repair writes.
    ///
    /// # Errors
    /// Propagates settled-image mismatch, I/O/barriers, cancellation and budgets.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
