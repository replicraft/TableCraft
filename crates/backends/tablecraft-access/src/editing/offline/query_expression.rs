//! Reviewed existing query metadata under the retained exclusive target owner.
use super::plan::MutationPlan;
mod fragment_plan;
mod staging;

pub(super) use fragment_plan::QueryExpressionPlan;

use super::{Failure, OfflineWriteSession, QueryChange, Receipt};
use std::path::Path;
use tablecraft_ace::RecoverableNativeQueryExpressionPlan;
use tablecraft_core::limits::ResourceBudget;

/// One reviewed existing-query request; no journal exists before consuming apply.
pub struct StagedQueryExpression {
    session: OfflineWriteSession,
    plan: QueryExpressionPlan,
}

impl OfflineWriteSession {
    /// Rebuild a reviewed query request from fresh bytes on the same retained target.
    /// Capture/native reconstruction spend `native`; full-source/image proof spends
    /// `proof`. No expression, parameter, provider or SQL execution is replayed.
    /// Hosts finish review/confirmation before acquiring this exclusive session.
    pub fn stage_reviewed_query_expression_with_budgets(
        self,
        reviewed: RecoverableNativeQueryExpressionPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedQueryExpression, Failure> {
        self.stage_query_expression(
            QueryExpressionPlan::Fitting(reviewed),
            native,
            proof,
            cancelled,
        )
    }
}

impl StagedQueryExpression {
    /// Exact original query name; no rename or namespace reassignment occurs.
    pub fn query(&self) -> &str {
        self.plan.query()
    }

    /// Original signed native query identity, separate from row/table identities.
    pub fn native_id(&self) -> i32 {
        self.plan.native_id()
    }

    /// Whether any submitted expression differs exactly from its original.
    pub fn changed(&self) -> bool {
        self.plan.changed()
    }

    /// Abandon unprepared staging while retaining this exact target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Commit this reviewed request through its own closed TCQE intent.
    /// Query completion stays separate from schema changes and record counts.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let changed = self.plan.changed().then_some(QueryChange::ExpressionSaved);
        super::apply::apply(
            self.session,
            MutationPlan::QueryExpression(self.plan),
            super::apply::CompletionFacts::Query(changed),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
