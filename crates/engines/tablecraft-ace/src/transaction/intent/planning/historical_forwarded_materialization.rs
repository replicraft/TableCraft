//! Relocated historical rows use a distinct request language and raw-variable proof.
use super::{RecoverableEditError, RecoverableNativeEditPlan};
use crate::{NativeFixedEditIntent, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Materialize one absent Number while retaining original stored-field bytes.
    ///
    /// Fresh source, serialization and owned-placement proofs precede the
    /// TCNI0014/profile14 binding. Earlier historical request languages remain closed.
    pub fn plan_recoverable_historical_forwarded_number_materialization(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!(
            "native.historical_forwarded_number_materialization.plan_recoverable"
        )
        .entered();
        let borrowed =
            crate::transaction::intent::fixed::borrow_groups(table, &groups, budget, cancelled)?;
        let plan = self.plan_historical_forwarded_number_materialization(
            table, &borrowed, budget, cancelled,
        )?;
        drop(borrowed);

        let original =
            NativeFixedEditIntent::new(table, groups, plan.before_bytes(), budget, cancelled)?;
        let intent = NativeUpdateIntent::from_historical_forwarded_number_materialization(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}
