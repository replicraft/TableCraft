//! Absent Number materialization retains its own original-request language.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{NativeFixedEditIntent, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Materialize one absent nullable Number field within existing owned DATA capacity.
    ///
    /// Complete native source, serialization and placement proofs precede the
    /// TCNI0012/profile12 binding. No physical target or forwarding pointer is
    /// serialized as authority; recovery must reconstruct the same full images.
    pub fn plan_recoverable_historical_number_materialization(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.historical_number_materialization.plan_recoverable")
                .entered();
        let borrowed =
            crate::transaction::intent::fixed::borrow_groups(table, &groups, budget, cancelled)?;
        let plan =
            self.plan_historical_number_materialization(table, &borrowed, budget, cancelled)?;
        drop(borrowed);

        let original =
            NativeFixedEditIntent::new(table, groups, plan.before_bytes(), budget, cancelled)?;
        let intent = NativeUpdateIntent::from_historical_number_materialization(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}
