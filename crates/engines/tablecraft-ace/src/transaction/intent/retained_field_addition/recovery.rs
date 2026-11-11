//! The original TCAC12 request requalifies the same immutable V3 journal session.
use super::{
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

#[must_use = "recover explicitly or retain the qualified V3 journal"]
pub struct QualifiedRetainedTypedFieldAdditionRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl Intent {
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedRetainedTypedFieldAdditionRecovery<'a, J>, Error> {
        let _span =
            tracing::debug_span!("native.retained_typed_field_addition_recovery.qualify").entered();
        self.verify_before(checked.before_bytes(), budget, cancelled)?;
        let image = NativeImage::capture_retained(
            &mut Cursor::new(checked.before_bytes()),
            scope,
            budget,
            cancelled,
        )?;
        let candidate = image.prepare_retained_typed_field_addition(
            self.table(),
            self.definition(),
            budget,
            cancelled,
        )?;
        equal_image(
            candidate.before_bytes(),
            checked.before_bytes(),
            budget,
            cancelled,
        )?;
        equal_image(
            candidate.after_bytes(),
            checked.after_bytes(),
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(QualifiedRetainedTypedFieldAdditionRecovery { checked })
    }
}

impl<J: DurableStorage> QualifiedRetainedTypedFieldAdditionRecovery<'_, J> {
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
