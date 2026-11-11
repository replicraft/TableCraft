//! Original TCAC14 reconstruction qualifies the same immutable checked V3 owner.
use super::{
    NativeUnadornedFieldAdditionIntent as Intent, UnadornedFieldAdditionIntentError as Error,
};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

/// The same checked journal session after complete original/effect reconstruction.
#[must_use = "recover explicitly or retain the qualified V3 journal"]
pub struct QualifiedUnadornedFieldAdditionRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl Intent {
    /// Rebuild the bound SQL declaration from complete original bytes before effects.
    /// No SQL, clock, default expression or older action decoder is evaluated.
    ///
    /// # Errors
    /// Refuses stale bindings, native source/identity or whole AFTER mismatch,
    /// allocation failure, cumulative resource exhaustion and cancellation.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedUnadornedFieldAdditionRecovery<'a, J>, Error> {
        let _span = tracing::debug_span!("native.unadorned_field_addition.qualify_v3").entered();
        check(budget, cancelled)?;
        self.verify_before(checked.before_bytes(), budget, cancelled)?;
        let image = NativeImage::capture_retained(
            &mut Cursor::new(checked.before_bytes()),
            scope,
            budget,
            cancelled,
        )?;
        let (effect, _) = self.rebuild_verified(image, budget, cancelled)?;
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
        Ok(QualifiedUnadornedFieldAdditionRecovery { checked })
    }
}

impl<J: DurableStorage> QualifiedUnadornedFieldAdditionRecovery<'_, J> {
    /// State observed from this exact qualified journal session.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover through the same checked owner; this does not reopen or reparse it.
    ///
    /// # Errors
    /// Retains journal target/storage/barrier, resource and cancellation failures.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm the completed target through this same qualified journal owner.
    ///
    /// # Errors
    /// Retains journal state, exact-image, storage, resource and cancellation errors.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
