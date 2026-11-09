//! Fresh retained-target staging shared by fitting and explicit fragment edits.
use super::{QueryExpressionPlan, StagedQueryExpression};
use crate::editing::offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState};
use tablecraft_ace::{NativeImage, RecoverableNativeQueryExpressionEditsPlan};
use tablecraft_core::limits::ResourceBudget;

impl OfflineWriteSession {
    /// Rebuild the exact ordered TCQE0002 requests from fresh retained BEFORE.
    /// No expression evaluation, SQL execution or journal creation occurs here.
    /// Capture/reconstruction and complete-image proof retain separate budgets.
    pub fn stage_reviewed_query_expression_edits_with_budgets(
        self,
        reviewed: RecoverableNativeQueryExpressionEditsPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedQueryExpression, Failure> {
        self.stage_query_expression(
            QueryExpressionPlan::Fragments(reviewed),
            native,
            proof,
            cancelled,
        )
    }

    pub(super) fn stage_query_expression(
        mut self,
        reviewed: QueryExpressionPlan,
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
            crate::editing::offline::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                proof,
                cancelled,
            )?;
            reviewed.requalify(image, native, proof, cancelled)
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
