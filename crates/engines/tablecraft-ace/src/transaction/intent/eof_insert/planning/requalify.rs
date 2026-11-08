//! Fresh source-bound qualification of the original eof_insert intent.
use super::RecoverableNativeEofInsertPlan;
use crate::transaction::{intent::insert_requalification, qualified_recovery::equal_image};
use crate::{NativeImage, NativeRecoveryQualificationError};
use tablecraft_core::limits::ResourceBudget;

impl RecoverableNativeEofInsertPlan {
    /// Rebuild this exact native family from original typed/presence requests.
    /// Immutable BEFORE/AFTER comparisons and intent binding spend `proof`;
    /// fresh normal native admission spends `native`. No SQL or host provider
    /// is evaluated, and the original intent is retained unchanged.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.eof_insert.requalify").entered();
        equal_image(fresh.as_bytes(), self.plan.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;

        let rebuilt = fresh.plan_ordinary_eof_row_inserts(
            self.intent.table(),
            self.intent.rows(),
            native,
            cancelled,
        )?;
        let plan = insert_requalification::finish(&self.plan, rebuilt, proof, cancelled)?;
        Ok(Self {
            plan,
            intent: self.intent,
        })
    }
}
