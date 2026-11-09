//! A sealed table DROP candidate retains its complete original request and source.
use super::{NativeTableDropIntent as Intent, TableDropIntentError as Error};
use crate::{
    NativeImage, NativeTableDropIdentity, NativeTableDropPlan, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// Owning native DROP candidate plus its distinct original-action record.
#[must_use = "prepare explicitly or retain the reviewed table DROP plan"]
pub struct RecoverableTableDropPlan {
    candidate: Box<NativeTableDropPlan>,
    intent: Intent,
}

impl NativeImage {
    /// Construct and bind the exact table DROP without acquiring write resources.
    ///
    /// # Errors
    /// Returns an error for unsupported source/dependencies, invalid target,
    /// cancellation, resource exhaustion or full-image binding failure.
    pub fn plan_recoverable_table_drop(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableTableDropPlan, Error> {
        let _span = tracing::debug_span!("native.table_drop.plan_recoverable").entered();
        let candidate = self.plan_table_drop(table, budget, cancelled)?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        RecoverableTableDropPlan::new(candidate, intent, budget, cancelled)
    }
}

impl RecoverableTableDropPlan {
    fn new(
        candidate: NativeTableDropPlan,
        intent: Intent,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        budget.charge_allocation(1, size_of::<NativeTableDropPlan>() as u64)?;
        Ok(Self {
            candidate: Box::new(candidate),
            intent,
        })
    }

    /// Bounded data-only original action, not journal authority.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete source captured by the native source proof.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete unpublished native replacement.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Exact original native table spelling.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Original semantic and physical native table identity.
    pub fn identity(&self) -> NativeTableDropIdentity {
        self.candidate.identity()
    }

    /// Source definition's stored row count, separate from a schema receipt.
    pub fn declared_rows(&self) -> u32 {
        self.candidate.declared_rows()
    }

    /// Rebuild the frozen original action from fresh bytes under independent
    /// native/proof pools. Neither a decoded AFTER nor a decoder retry is accepted.
    ///
    /// # Errors
    /// Returns an error for source drift, changed identity/profile, different
    /// candidate bytes, cancellation or exhausted resources.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.table_drop.requalify").entered();
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

    /// Prepare only the exact equal-length pair using the checked V1 journal.
    /// Hosts retain exclusive association and durable namespace lifecycle ownership.
    ///
    /// # Errors
    /// Returns a journal error for different current bytes, storage failure,
    /// cancellation, nonempty journal or exhausted resources.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.table_drop.prepare_v1").entered();
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
