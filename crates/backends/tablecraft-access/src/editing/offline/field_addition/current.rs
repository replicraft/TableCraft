//! Current metadata is freshly requalified through the retained target owner.
use super::{OfflineWriteSession, StagedFieldAddition, plan::Plan};
use crate::editing::offline::Failure;
use tablecraft_ace::RecoverableCurrentFieldAdditionPlan;
use tablecraft_core::limits::ResourceBudget;

impl OfflineWriteSession {
    /// Requalify the complete source and fixed Number declaration before staging.
    /// Refusal retains the original owner and creates no journal namespace.
    pub fn stage_reviewed_current_field_addition_with_budgets(
        self,
        reviewed: RecoverableCurrentFieldAdditionPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_current_field_addition").entered();
        self.stage_field_addition(Plan::Current(Box::new(reviewed)), native, proof, cancelled)
    }
}
