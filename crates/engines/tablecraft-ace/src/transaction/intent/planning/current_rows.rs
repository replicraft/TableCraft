//! One original-request language for current Number/Text row updates.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{
    EditError, NativeFittedEditIntent, NativeFittedIntentGroup, NativeImage, NativeUpdateIntent,
    WriteBlocker,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Reconstruct current sparse rows from their actual native identities and
    /// stored slots. The original expected/replacement carriers and entire BEFORE
    /// bind TCNI0016; decoded data never carries a physical write permission.
    pub fn plan_recoverable_current_row_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.current_rows.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        if !self.current_row_updates_selected(table, &borrowed, budget, cancelled)? {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout).into());
        }
        let plan = self.plan_current_row_updates(table, &borrowed, budget, cancelled)?;
        drop(borrowed);
        Ok(RecoverableNativeEditPlan {
            plan,
            intent: NativeUpdateIntent::from_current_rows(original),
        })
    }
}
