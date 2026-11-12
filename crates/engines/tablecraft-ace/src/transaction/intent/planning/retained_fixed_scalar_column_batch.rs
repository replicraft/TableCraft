//! TCNI28 binds an original atomic typed fixed-scalar column batch before native admission.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{
    EditError, NativeFittedEditIntent, NativeFittedIntentGroup, NativeImage, NativeIntentError,
    NativeUpdateIntent, WriteBlocker, header::check,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Bind every original typed fixed-scalar witness and build one atomic native plan.
    ///
    /// # Errors
    /// Rejects invalid originals or unsupported source/program/constraint/index,
    /// column, row or placement shapes. Source conflicts, preservation mismatch,
    /// allocation failure, cancellation and caller-work exhaustion also fail
    /// without publishing any native effect.
    pub fn plan_recoverable_retained_fixed_scalar_column_batch_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.retained_fixed_scalar_column_batch.plan_recoverable")
                .entered();
        let [group] = groups.as_slice() else {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout).into());
        };
        let requested = group
            .fixed_scalar_kind()
            .ok_or(NativeIntentError::UnsupportedProfile)?;
        let kind =
            self.retained_fixed_scalar_column_kind(table, group.column(), budget, cancelled)?;
        if requested != kind {
            return Err(EditError::Unsupported(WriteBlocker::Column).into());
        }

        let original = NativeFittedEditIntent::new_retained_fixed_scalar_column_batch(
            table,
            groups,
            self.as_bytes(),
            budget,
            cancelled,
        )?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan = self.plan_retained_fixed_scalar_column_batch_updates(
            table, &borrowed, kind, budget, cancelled,
        )?;
        if plan.before_bytes().len() != plan.after_bytes().len() {
            return Err(EditError::Preservation.into());
        }
        drop(borrowed);
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeEditPlan {
            plan,
            intent: NativeUpdateIntent::from_retained_fixed_scalar_column_batch(original)?,
        })
    }
}
