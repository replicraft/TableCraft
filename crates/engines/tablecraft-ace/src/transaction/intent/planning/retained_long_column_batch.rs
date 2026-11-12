//! TCNI25 binds an original atomic LONG column batch before native admission.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{
    EditError, NativeFittedEditIntent, NativeFittedIntentGroup, NativeImage, NativeUpdateIntent,
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Bind all original expected/replacement cells and build one atomic native plan.
    pub fn plan_recoverable_retained_long_column_batch_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.retained_long_column_batch.plan_recoverable").entered();
        let original = NativeFittedEditIntent::new_retained_long_column_batch(
            table,
            groups,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan =
            self.plan_retained_long_column_batch_updates(table, &borrowed, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len() {
            return Err(EditError::Preservation.into());
        }
        drop(borrowed);
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeEditPlan {
            plan,
            intent: NativeUpdateIntent::from_retained_long_column_batch(original),
        })
    }
}
