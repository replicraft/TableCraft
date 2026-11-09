//! Distinct reviewed TCAC6 admission through the existing target capability.
use super::{OfflineWriteSession, StagedFieldAddition, plan::Plan};
use crate::editing::offline::Failure;
use tablecraft_ace::RecoverableFieldAdditionAfterOffPlan;
use tablecraft_core::limits::ResourceBudget;

impl OfflineWriteSession {
    /// Reconstruct the complete ADD-after-OFF source and target before staging.
    /// Refusal retains the same owner and creates no journal namespace.
    pub fn stage_reviewed_field_addition_after_off_with_budgets(
        self,
        reviewed: RecoverableFieldAdditionAfterOffPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_field_addition_after_off")
            .entered();
        self.stage_field_addition(Plan::AfterOff(reviewed), native, proof, cancelled)
    }
}
