//! A proved metadata effect and moved original declaration share no copied payload.
use super::{
    DeclaredFieldAdditionIntentError as Error, NativeDeclaredFieldAdditionIntent as Intent,
};
use crate::{
    DeclaredFieldAdditionDefinition, InspectError, NativeImage, header::check,
    table::DeclaredFieldAdditionEffect, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Source-qualified SQL ADD effect paired with its exact original declaration.
pub struct RecoverableDeclaredFieldAdditionPlan {
    effect: Box<DeclaredFieldAdditionEffect>,
    intent: Intent,
}

fn boxed(
    effect: DeclaredFieldAdditionEffect,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Box<DeclaredFieldAdditionEffect>, Error> {
    budget
        .charge_allocation(1, size_of::<DeclaredFieldAdditionEffect>() as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    Ok(Box::new(effect))
}

impl NativeImage {
    /// Prepare one unadorned Currency ADD and retain its moved original under TCAC13.
    /// This constructs no journal and does not execute SQL or default expressions.
    ///
    /// # Errors
    /// Retains declaration, source/program/dependency, namespace, physical capacity,
    /// image proof, allocation, cumulative-work and cancellation failures.
    pub fn plan_recoverable_retained_declared_field_addition(
        self,
        table: &str,
        definition: DeclaredFieldAdditionDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableDeclaredFieldAdditionPlan, Error> {
        let _span = tracing::debug_span!("native.declared_field_addition.plan").entered();
        let candidate =
            self.prepare_retained_declared_field_addition(table, definition, budget, cancelled)?;
        let (effect, intent) = Intent::from_candidate(candidate, budget, cancelled)?;
        Ok(RecoverableDeclaredFieldAdditionPlan {
            effect: boxed(effect, budget, cancelled)?,
            intent,
        })
    }
}

impl RecoverableDeclaredFieldAdditionPlan {
    /// Exact original data, never final physical descriptor bytes.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    /// Actual source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.intent.table()
    }
    /// Actual new field spelling retained by the original declaration.
    pub fn column(&self) -> &str {
        self.intent.column()
    }
    /// Exact Currency declaration with every optional property absent.
    pub fn definition(&self) -> &DeclaredFieldAdditionDefinition {
        self.intent.definition()
    }
    /// Allocated native field ordinal, separate from display position.
    pub fn native_number(&self) -> usize {
        usize::from(self.intent.native_number())
    }
    /// Allocated stored creation-time field identity.
    pub fn column_id(&self) -> u16 {
        self.intent.column_id()
    }
    /// Replacement active field count, separate from native high-water counts.
    pub fn column_count(&self) -> usize {
        usize::from(self.intent.column_count())
    }
    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.effect.before_bytes()
    }
    /// Complete independently proved replacement image.
    pub fn after_bytes(&self) -> &[u8] {
        self.effect.after_bytes()
    }
    /// Share the already paid immutable replacement with a readonly save witness.
    /// This does not copy its payload or grant native write authority.
    pub fn after_shared_bytes(&self) -> bytes::Bytes {
        self.effect.after_shared_bytes()
    }

    /// Reconstruct the exact original from a fresh exclusive-source capture.
    /// Native/source work and complete image/binding proof retain separate pools.
    ///
    /// # Errors
    /// Refuses source/binding/identity or whole AFTER drift, unsupported source,
    /// allocation, cumulative work exhaustion and cancellation before persistence.
    pub fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let Self { effect, intent } = self;
        equal_image(image.as_bytes(), effect.before_bytes(), proof, cancelled)?;
        intent.verify_before(image.as_bytes(), proof, cancelled)?;
        let expected_after = effect.after_shared_bytes();
        drop(effect);
        let (rebuilt, intent) = intent.rebuild_verified(image, native, cancelled)?;
        equal_image(rebuilt.after_bytes(), &expected_after, proof, cancelled)?;
        check(proof, cancelled)?;
        Ok(Self {
            effect: boxed(rebuilt, native, cancelled)?,
            intent,
        })
    }

    /// Prepare only the existing checked V3 expected-image protocol.
    /// The host retains exclusive resources and durable namespace ownership.
    ///
    /// # Errors
    /// Returns the journal's exact-original, framing, storage, allocation,
    /// resource and cancellation errors without a legacy-protocol retry.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.declared_field_addition.prepare_v3").entered();
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
