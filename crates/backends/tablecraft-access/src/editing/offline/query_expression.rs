//! Reviewed existing query metadata under the retained exclusive target owner.
use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, QueryChange, Receipt, SessionState};
use std::path::Path;
use tablecraft_ace::{NativeImage, RecoverableNativeQueryExpressionPlan};
use tablecraft_core::limits::ResourceBudget;

/// One reviewed existing-query request; no journal exists before consuming apply.
pub struct StagedQueryExpression {
    session: OfflineWriteSession,
    plan: RecoverableNativeQueryExpressionPlan,
}

impl OfflineWriteSession {
    /// Rebuild a reviewed query request from fresh bytes on the same retained target.
    /// Capture/native reconstruction spend `native`; full-source/image proof spends
    /// `proof`. No expression, parameter, provider or SQL execution is replayed.
    /// Hosts finish review/confirmation before acquiring this exclusive session.
    pub fn stage_reviewed_query_expression_with_budgets(
        mut self,
        reviewed: RecoverableNativeQueryExpressionPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedQueryExpression, Failure> {
        let _span =
            tracing::debug_span!("access.offline.stage_reviewed_query_expression").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                native,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            super::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                proof,
                cancelled,
            )?;
            reviewed
                .requalify_with_budgets(image, native, proof, cancelled)
                .map_err(Cause::QueryExpressionQualification)
        })();
        match result {
            Ok(plan) => Ok(StagedQueryExpression {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
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

    /// Whether the requested literal differs exactly from the original literal.
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
