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

/// The same immutable V3 session after native reconstruction of its TCAC12 original.
#[must_use = "recover explicitly or retain the qualified V3 journal"]
pub struct QualifiedRetainedTypedFieldAdditionRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl Intent {
    /// Reconstruct the original ADD and compare both complete V3 image witnesses.
    ///
    /// # Errors
    /// Refuses changed originals or forged replacements, unsupported native
    /// declarations/source effects, allocation failure, budget exhaustion or cancellation.
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
    /// Checked V3 framing state; this does not replace native source qualification.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover through this exact checked session and caller-owned target.
    ///
    /// # Errors
    /// Returns storage, original/replacement mismatch, resource or cancellation
    /// failures under the journal's documented recovery semantics.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm settled original/replacement bytes and barriers without target writes.
    ///
    /// # Errors
    /// Returns unsettled state, byte mismatch, storage, resource or cancellation errors.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
