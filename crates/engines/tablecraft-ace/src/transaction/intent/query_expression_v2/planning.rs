//! Complete source/request reconstruction over the existing fixed-image journal.
use super::{NativeQueryExpressionEditsIntent, QueryExpressionEditsIntentError};
use crate::{
    NativeImage, NativeQueryExpressionEditsPlan, QueryFragmentExpressionEdit, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedImage, RecoveryOutcome,
};

/// Sealed candidate and original request; neither arbitrary AFTER nor ROW input.
pub struct RecoverableNativeQueryExpressionEditsPlan {
    pub(super) plan: NativeQueryExpressionEditsPlan,
    pub(super) intent: NativeQueryExpressionEditsIntent,
}

impl NativeImage {
    /// Bind the distinct native expression candidate to the entire original image.
    pub fn plan_recoverable_query_expression_edits(
        self,
        query: &str,
        edits: &[QueryFragmentExpressionEdit<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeQueryExpressionEditsPlan, QueryExpressionEditsIntentError> {
        let _span =
            tracing::debug_span!("native.query_expression_edits.plan_recoverable").entered();
        let plan = self.plan_query_expression_edits(query, edits, budget, cancelled)?;
        let intent = NativeQueryExpressionEditsIntent::capture(&plan, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeQueryExpressionEditsPlan { plan, intent })
    }
}

impl RecoverableNativeQueryExpressionEditsPlan {
    /// Original exact ordered request; encoding it does not grant permission.
    pub fn intent(&self) -> &NativeQueryExpressionEditsIntent {
        &self.intent
    }

    /// Complete immutable original bytes retained by the native planner.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete independently rebuilt candidate, without replacement constructors.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Exact original source-resolved query name.
    pub fn query(&self) -> &str {
        self.plan.query()
    }

    /// Exact signed native query identity, independent of caller catalog scope.
    pub fn native_id(&self) -> i32 {
        self.plan.native_id()
    }

    /// Exact no-ops retain full original source, definition and placement checks.
    pub fn changed(&self) -> bool {
        self.plan.changed()
    }

    /// Reconstruct native authority from fresh complete BEFORE under separate pools.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionEditsIntentError> {
        let _span = tracing::debug_span!("native.query_expression_edits.requalify").entered();
        equal_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;

        let plan = self.intent.reconstruct(fresh, native, cancelled)?;
        self.intent.verify_plan(&plan, native, cancelled)?;
        equal_image(plan.before_bytes(), self.before_bytes(), proof, cancelled)?;
        equal_image(plan.after_bytes(), self.after_bytes(), proof, cancelled)?;
        check(proof, cancelled)?;
        Ok(Self {
            plan,
            intent: self.intent,
        })
    }

    /// Prepare only the exact original/candidate over caller-owned paired resources.
    /// TCQE0002 is a semantic request version; the journal remains fixed-image V1.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedQueryExpressionEdits<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.query_expression_edits.prepare").entered();
        let prepared = journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedQueryExpressionEdits { prepared })
    }
}

/// One retained fixed-image native expression transition over caller resources.
#[must_use = "commit or explicitly recover the retained expression-edits journal"]
pub struct PreparedQueryExpressionEdits<'a, D, J> {
    prepared: PreparedImage<'a, D, J>,
}

impl<D: DurableStorage, J: DurableStorage> PreparedQueryExpressionEdits<'_, D, J> {
    /// Commit the exact qualified images; failure retains generic recovery needs.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        self.prepared.commit(budget, cancelled)
    }

    /// Restore and synchronize the complete original through the same V1 session.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.prepared.rollback(budget, cancelled)
    }
}
