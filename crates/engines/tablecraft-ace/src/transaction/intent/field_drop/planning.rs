//! Owning saved-field DROP plus original request, never decoded write authority.
mod retained;

use super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent, candidate::Candidate};
use crate::{
    NativeFieldDropIdentity, NativeImage, SavedFieldDropSourcePurpose, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// Sealed exact saved-field DROP candidate, complete source and original request.
pub struct RecoverableFieldDropPlan {
    candidate: Candidate,
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
        let candidate = Candidate::empty(
            self.prepare_saved_field_drop(table, column, budget, cancelled)?,
            budget,
            cancelled,
        )?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldDropPlan::new(candidate, intent, budget, cancelled)
    }

    /// Bind the separate current populated-tail request after native qualification.
    pub fn plan_recoverable_current_tail_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldDropPlan, Error> {
        let _span =
            tracing::debug_span!("native.current_tail_field_drop.plan_recoverable").entered();
        let candidate = Candidate::populated_tail(
            self.prepare_current_tail_field_drop(table, column, budget, cancelled)?,
            budget,
            cancelled,
        )?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldDropPlan::new(candidate, intent, budget, cancelled)
    }

    /// Select one source language from original metadata before its full proof.
    /// No refused DROP is retried through another implementation.
    pub fn plan_recoverable_field_drop(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldDropPlan, Error> {
        if self.has_retained_table_source(table, budget, cancelled)? {
            if self.select_native_retained_typed_tail_field_drop_source(
                table, column, budget, cancelled,
            )? {
                return self.plan_recoverable_retained_typed_tail_field_drop(
                    table, column, budget, cancelled,
                );
            }
            return self
                .plan_recoverable_retained_tail_field_drop(table, column, budget, cancelled);
        }
        match self.field_drop_source_purpose(table, budget, cancelled)? {
            SavedFieldDropSourcePurpose::Empty => {
                self.plan_recoverable_saved_field_drop(table, column, budget, cancelled)
            }
            SavedFieldDropSourcePurpose::PopulatedTail => {
                self.plan_recoverable_current_tail_field_drop(table, column, budget, cancelled)
            }
        }
    }
}

impl RecoverableFieldDropPlan {
    fn new(
        candidate: Candidate,
        intent: Intent,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        Ok(Self { candidate, intent })
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
        if self.intent.is_retained() || self.intent.is_retained_typed() {
            return Err(JournalError::ImageSize);
        }
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
