//! Owning saved-field DROP plus original request, never decoded write authority.
use super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent};
use crate::{
    NativeFieldDropCandidate, NativeFieldDropIdentity, NativeImage, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// Sealed exact saved-field DROP candidate, complete source and original request.
pub struct RecoverableFieldDropPlan {
    candidate: Box<NativeFieldDropCandidate>,
    intent: Intent,
}

impl NativeImage {
    /// Bind the original DROP request and original native identity to the whole BEFORE.
    /// This does not open a journal or acquire a native save owner.
    pub fn plan_recoverable_saved_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldDropPlan, Error> {
        let _span = tracing::debug_span!("native.field_drop.plan_recoverable").entered();
        let candidate = self.prepare_saved_field_drop(table, column, budget, cancelled)?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldDropPlan::new(candidate, intent, budget, cancelled)
    }
}

impl RecoverableFieldDropPlan {
    fn new(
        candidate: NativeFieldDropCandidate,
        intent: Intent,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        budget.charge_allocation(1, size_of::<NativeFieldDropCandidate>() as u64)?;

        Ok(Self {
            candidate: Box::new(candidate),
            intent,
        })
    }

    /// Bounded original request data, never serialized storage authority.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete original from the sealed native source proof.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete native candidate reconstructed from the exact request.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Actual original native field spelling.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }

    /// Fresh original native identity, including separate number and stored ID.
    pub fn identity(&self) -> NativeFieldDropIdentity {
        self.candidate.identity()
    }

    /// Rebuild the original request from fresh bytes under separate cumulative
    /// native and proof pools; no caller-supplied replacement or decoder retry.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.field_drop.requalify").entered();
        equal_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate = self.intent.reconstruct(fresh, native, cancelled)?;
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
        check(proof, cancelled)?;
        Self::new(candidate, self.intent, native, cancelled)
    }

    /// Prepare the exact equal-length pair in a checked V1 journal using only
    /// caller-owned target/journal resources. Hosts own exclusive association,
    /// durable namespace setup and cleanup; no other journal version is tried.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.field_drop.prepare_v1").entered();
        journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}
