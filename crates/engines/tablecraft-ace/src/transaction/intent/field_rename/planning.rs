//! Owning candidate plus original request; no constructor from decoded authority.
use super::{FieldRenameIntentError as Error, NativeFieldRenameIntent as Intent};
use crate::{
    NativeFieldRenameCandidate, NativeFieldRenameIdentity, NativeImage, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// Sealed exact field-rename candidate, complete source and original request.
pub struct RecoverableFieldRenamePlan {
    candidate: NativeFieldRenameCandidate,
    intent: Intent,
}

impl NativeImage {
    /// Bind the same-width request and original native identity to the whole BEFORE.
    /// This does not open a journal or acquire a native save owner.
    pub fn plan_recoverable_same_width_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.field_rename.plan_recoverable").entered();
        let candidate =
            self.prepare_same_width_field_rename(table, column, new_name, budget, cancelled)?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableFieldRenamePlan { candidate, intent })
    }
}

impl RecoverableFieldRenamePlan {
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

    /// Exact requested spelling, retaining unchanged and case-only distinctions.
    pub fn new_name(&self) -> &str {
        self.candidate.new_name()
    }

    /// Fresh original native identity, including separate number and stored ID.
    pub fn identity(&self) -> NativeFieldRenameIdentity {
        self.candidate.identity()
    }

    /// Fully qualified no-op fact. Hosts may skip persistence for this request.
    pub fn changed(&self) -> bool {
        self.candidate.changed()
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
        let _span = tracing::debug_span!("native.field_rename.requalify").entered();
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
        Ok(Self {
            candidate,
            intent: self.intent,
        })
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
        let _span = tracing::debug_span!("native.field_rename.prepare_v1").entered();
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
