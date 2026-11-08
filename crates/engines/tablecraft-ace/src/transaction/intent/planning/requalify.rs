//! Reviewed UPDATE validation separates immutable proof work from native rebuilding.
use super::RecoverableNativeEditPlan;
use crate::transaction::{
    NativeRecoveryQualificationError, qualified_recovery::equal_image, update_rebuild,
};
use crate::{NativeEditPlan, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

impl RecoverableNativeEditPlan {
    /// Requalify this exact original request against a fresh immutable capture.
    /// No SQL, journal or target is evaluated here. Both complete images and the
    /// effective change count must reproduce the reviewed plan before it returns.
    /// All validation and native reconstruction share the supplied budget.
    pub fn requalify(
        self,
        fresh: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.update.requalify").entered();
        equal_image(
            fresh.as_bytes(),
            self.plan.before_bytes(),
            budget,
            cancelled,
        )?;
        let plan = update_rebuild::rebuild(fresh, &self.intent, budget, cancelled)?;
        self.finish_requalification(plan, budget, cancelled)
    }

    /// Requalify with independently bounded native and immutable-proof work.
    /// Complete image comparisons and original-intent binding spend `proof`;
    /// fresh native reconstruction spends `native`. Neither allowance transfers
    /// into the other, and both retain cancellation checks. Capture is separate.
    /// No SQL is reevaluated and no durable target or journal is accepted.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.update.requalify").entered();
        equal_image(fresh.as_bytes(), self.plan.before_bytes(), proof, cancelled)?;
        check(proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;

        let plan =
            update_rebuild::rebuild_verified_binding(fresh, &self.intent, native, cancelled)?;
        self.finish_requalification(plan, proof, cancelled)
    }

    fn finish_requalification(
        self,
        plan: NativeEditPlan,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        equal_image(
            plan.before_bytes(),
            self.plan.before_bytes(),
            proof,
            cancelled,
        )?;
        equal_image(
            plan.after_bytes(),
            self.plan.after_bytes(),
            proof,
            cancelled,
        )?;

        if plan.changed_rows() != self.plan.changed_rows() {
            return Err(NativeRecoveryQualificationError::ImageMismatch);
        }

        check(proof, cancelled)?;

        Ok(Self {
            plan,
            intent: self.intent,
        })
    }
}
