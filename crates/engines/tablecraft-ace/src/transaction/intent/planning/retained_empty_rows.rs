//! Original retained empty target groups, bound before consuming native admission.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{
    EditError, NativeFittedEditIntent, NativeFittedIntentGroup, NativeImage, NativeUpdateIntent,
    WriteBlocker, header::check,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Bind TCNI24 named zero-assignment fixed groups to this complete image.
    /// Metadata selects the route; the native planner rechecks its bound source.
    ///
    /// # Errors
    /// Refuses invalid originals, unsupported source/targets, failed whole-image
    /// identity proof, allocation/work exhaustion and cancellation.
    pub fn plan_recoverable_retained_empty_row_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.retained_empty_rows.plan_recoverable").entered();
        if !self
            .has_retained_table_source(table, budget, cancelled)
            .map_err(EditError::from)?
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout).into());
        }
        let original = NativeFittedEditIntent::new_retained_empty(
            table,
            groups,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan = self.plan_retained_empty_row_updates(table, &borrowed, budget, cancelled)?;
        if plan.changed_rows() != 0 || plan.before_bytes().len() != plan.after_bytes().len() {
            return Err(EditError::Preservation.into());
        }
        drop(borrowed);
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeEditPlan {
            plan,
            intent: NativeUpdateIntent::from_retained_empty_rows(original),
        })
    }
}
