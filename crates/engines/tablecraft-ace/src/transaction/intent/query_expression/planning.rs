//! Sealed original-request reconstruction before fixed-image journal preparation.
use super::{NativeQueryExpressionIntent, QueryExpressionIntentError};
use crate::{
    InspectError, NativeImage, NativeQueryExpressionPlan, NativeRecoveryQualificationError,
    QueryExpressionError, header::check, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, CommitError, DurableStorage, JournalError, PreparedImage, RecoveryOutcome,
};
use thiserror::Error;

/// Original native candidate or intent capture failure before persistence.
#[derive(Debug, Error)]
pub enum RecoverableQueryExpressionError {
    /// Native cancellation, allocation or resource exhaustion.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Original request or complete-source binding could not be captured.
    #[error(transparent)]
    Intent(#[from] QueryExpressionIntentError),
    /// The selected native definition or storage is unqualified.
    #[error(transparent)]
    Expression(#[from] QueryExpressionError),
}

/// Failure before a freshly reconstructed plan or journal gains query authority.
#[derive(Debug, Error)]
pub enum QueryExpressionRecoveryError {
    /// Native cancellation, source capture, allocation or resource exhaustion.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Original query intent, native identity or source binding is unqualified.
    #[error(transparent)]
    Intent(#[from] QueryExpressionIntentError),
    /// Fresh original definition/storage failed the closed query planner.
    #[error(transparent)]
    Expression(#[from] QueryExpressionError),
    /// Complete byte-image proof differs or exhausts its independent pool.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
}

/// Qualified existing-query candidate retaining exact original intent and source.
/// There is no constructor from decoded intent or arbitrary replacement bytes.
pub struct RecoverableNativeQueryExpressionPlan {
    pub(super) plan: NativeQueryExpressionPlan,
    pub(super) intent: NativeQueryExpressionIntent,
}

impl NativeImage {
    /// Bind a native fitting constant SELECT request to the entire original image.
    pub fn plan_recoverable_query_literal_expression(
        self,
        query: &str,
        expected: &str,
        replacement: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeQueryExpressionPlan, RecoverableQueryExpressionError> {
        let _span = tracing::debug_span!("native.query_expression.plan_recoverable").entered();
        let plan =
            self.plan_query_literal_expression(query, expected, replacement, budget, cancelled)?;
        let intent = NativeQueryExpressionIntent::capture(&plan, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeQueryExpressionPlan { plan, intent })
    }
}

impl RecoverableNativeQueryExpressionPlan {
    /// Original request bound to this plan; encoding does not grant mutation authority.
    pub fn intent(&self) -> &NativeQueryExpressionIntent {
        &self.intent
    }

    /// Exact complete original retained during native qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete independently rebuilt candidate, not an arbitrary replacement.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Original native query name, unchanged by this operation.
    pub fn query(&self) -> &str {
        self.plan.query()
    }

    /// Exact signed persistent query ID, independent of caller CatalogScope.
    pub fn native_id(&self) -> i32 {
        self.plan.native_id()
    }

    /// Exact no-op requests retain full source and identity admission.
    pub fn changed(&self) -> bool {
        self.plan.changed()
    }

    /// Rebuild the same operation from fresh original bytes under independent pools.
    /// Persisted final fragments, SQL evaluation and patched AFTER images are unused.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryExpressionRecoveryError> {
        let _span = tracing::debug_span!("native.query_expression.requalify").entered();
        equal_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let plan = self.intent.reconstruct(fresh, native, cancelled)?;
        self.intent.verify_plan(&plan, proof, cancelled)?;
        equal_image(plan.before_bytes(), self.before_bytes(), proof, cancelled)?;
        equal_image(plan.after_bytes(), self.after_bytes(), proof, cancelled)?;
        check(proof, cancelled)?;
        Ok(Self {
            plan,
            intent: self.intent,
        })
    }

    /// Prepare only against the exact original target with caller-owned resources.
    /// Hosts retain exclusive offline pairing, durable namespaces and cleanup.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedQueryExpression<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.query_expression.prepare").entered();
        let prepared = journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )?;
        Ok(PreparedQueryExpression { prepared })
    }
}

/// One prepared native query transition over caller-owned paired V1 resources.
#[must_use = "commit or explicitly recover the retained query-expression journal"]
pub struct PreparedQueryExpression<'a, D, J> {
    prepared: PreparedImage<'a, D, J>,
}

impl<D: DurableStorage, J: DurableStorage> PreparedQueryExpression<'_, D, J> {
    /// Commit the checked original/candidate pair; failure retains recovery needs.
    pub fn commit(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CommitError> {
        self.prepared.commit(budget, cancelled)
    }

    /// Restore and synchronize the complete original using the same V1 session.
    pub fn rollback(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.prepared.rollback(budget, cancelled)
    }
}
