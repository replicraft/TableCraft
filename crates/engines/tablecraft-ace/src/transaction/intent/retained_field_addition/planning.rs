//! Original single-column requests own the independently proven native candidate.
use super::{
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use crate::{
    NativeImage, NativeRetainedTypedFieldAdditionCandidate, RetainedFieldAdditionDefinition,
    header::check, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Source-qualified typed ADD candidate paired with its immutable original request.
pub struct RecoverableRetainedTypedFieldAdditionPlan {
    candidate: Box<NativeRetainedTypedFieldAdditionCandidate>,
    intent: Intent,
}

impl NativeImage {
    /// Bind one real typed column before consuming the retained source proof.
    ///
    /// # Errors
    /// Returns typed original-binding, declaration, source/dependency/placement,
    /// independent image-proof, allocation, budget or cancellation failures.
    pub fn plan_recoverable_retained_typed_field_addition(
        self,
        table: &str,
        definition: RetainedFieldAdditionDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableRetainedTypedFieldAdditionPlan, Error> {
        let _span = tracing::debug_span!("native.retained_typed_field_addition.plan").entered();
        let intent = Intent::new(table, definition, self.as_bytes(), budget, cancelled)?;
        let candidate = self.prepare_retained_typed_field_addition(
            table,
            intent.definition(),
            budget,
            cancelled,
        )?;
        budget
            .charge_allocation(
                1,
                size_of::<NativeRetainedTypedFieldAdditionCandidate>() as u64,
            )
            .map_err(crate::InspectError::from)?;
        check(budget, cancelled)?;
        Ok(RecoverableRetainedTypedFieldAdditionPlan {
            candidate: Box::new(candidate),
            intent,
        })
    }
}

impl RecoverableRetainedTypedFieldAdditionPlan {
    /// Exact immutable original request, never replacement bytes.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    /// Exact original table spelling.
    pub fn table(&self) -> &str {
        self.intent.table()
    }
    /// Exact newly declared field spelling.
    pub fn column(&self) -> &str {
        self.intent.column()
    }
    /// Validated original type, policies and literal default.
    pub fn definition(&self) -> &RetainedFieldAdditionDefinition {
        self.intent.definition()
    }
    /// Complete immutable original image covered by the request binding.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }
    /// Complete independently proven replacement image.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Rebuild from a fresh complete source under separate native and proof pools.
    ///
    /// # Errors
    /// Refuses changed originals, different reconstructed effects, unsupported
    /// source/placement, allocation failure, exhausted budgets or cancellation.
    pub fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        equal_image(image.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(image.as_bytes(), proof, cancelled)?;
        let candidate = image.prepare_retained_typed_field_addition(
            self.table(),
            self.definition(),
            native,
            cancelled,
        )?;
        equal_image(
            candidate.before_bytes(),
            self.before_bytes(),
            proof,
            cancelled,
        )?;
        equal_image(
            candidate.after_bytes(),
            self.after_bytes(),
            proof,
            cancelled,
        )?;
        native
            .charge_allocation(
                1,
                size_of::<NativeRetainedTypedFieldAdditionCandidate>() as u64,
            )
            .map_err(crate::InspectError::from)?;
        check(native, cancelled)?;
        Ok(Self {
            candidate: Box::new(candidate),
            intent: self.intent,
        })
    }

    /// Prepare only this original TCAC12 action through the same V3 journal owner.
    ///
    /// # Errors
    /// Returns the journal's exact-original, framing, storage, resource or
    /// cancellation failure. Preparation does not commit the target.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.retained_typed_field_addition.prepare").entered();
        journal::prepare_resizable_v3_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}
