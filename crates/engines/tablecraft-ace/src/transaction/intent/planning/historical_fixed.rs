//! Actual original field presence selects one closed historical operation.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{NativeFixedIntentGroup, NativeImage};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Select existing-field editing or absent Number materialization before planning.
    ///
    /// Both constructors requalify their own source and operation. A selected
    /// operation's refusal is returned directly, without trying the other route.
    pub fn plan_recoverable_historical_fixed_updates(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.historical_fixed.select_recoverable").entered();
        let (materialize, mixed) = {
            let borrowed = crate::transaction::intent::fixed::borrow_groups(
                table, &groups, budget, cancelled,
            )?;
            let materialize = self
                .historical_number_materialization_selected(table, &borrowed, budget, cancelled)?;
            let mixed = materialize
                && self.historical_mixed_number_materialization_selected(
                    table, &borrowed, budget, cancelled,
                )?;
            (materialize, mixed)
        };

        if mixed {
            self.plan_recoverable_historical_mixed_number_materialization(
                table, groups, budget, cancelled,
            )
        } else if materialize {
            self.plan_recoverable_historical_number_materialization(
                table, groups, budget, cancelled,
            )
        } else {
            self.plan_recoverable_historical_present_fixed_updates(table, groups, budget, cancelled)
        }
    }
}
