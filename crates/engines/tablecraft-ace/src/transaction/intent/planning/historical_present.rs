//! Source-qualified present-field requests retain a separate recovery language.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{NativeFixedEditIntent, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Plan existing Int32 fields without expanding historical records.
    ///
    /// The source and every selected field are requalified before binding the
    /// original requests as TCNI0011/profile11. Absent fields and old recovery
    /// languages do not acquire this operation's authority.
    pub fn plan_recoverable_historical_present_fixed_updates(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span =
            tracing::debug_span!("native.historical_present_fixed.plan_recoverable").entered();
        let borrowed =
            crate::transaction::intent::fixed::borrow_groups(table, &groups, budget, cancelled)?;
        let plan =
            self.plan_historical_present_fixed_updates(table, &borrowed, budget, cancelled)?;
        drop(borrowed);

        let original =
            NativeFixedEditIntent::new(table, groups, plan.before_bytes(), budget, cancelled)?;
        let intent = NativeUpdateIntent::from_historical_present_fixed(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}
