//! Historical Short Text updates retain original carriers and their native proof.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{
    EditError, NativeFittedEditIntent, NativeFittedIntentGroup, NativeImage, NativeUpdateIntent,
    WriteBlocker,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Update one Short Text field without changing other historical payloads.
    /// TCNI0015 binds original NULL/Text requests; no physical placement is trusted
    /// from the record, and recovery independently reconstructs the full candidate.
    pub fn plan_recoverable_historical_short_text_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.historical_short_text_updates.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        if !self.historical_short_text_updates_selected(table, &borrowed, budget, cancelled)? {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout).into());
        }
        let plan = self.plan_historical_short_text_updates(table, &borrowed, budget, cancelled)?;
        drop(borrowed);
        let intent = NativeUpdateIntent::from_historical_short_text_updates(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}
