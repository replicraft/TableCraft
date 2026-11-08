//! Fresh native reconstruction of one sealed DELETE candidate, never SQL replay.
use super::RecoverableNativeDeletePlan;
use crate::transaction::{NativeRecoveryQualificationError, qualified_recovery::equal_image};
use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

impl RecoverableNativeDeletePlan {
    /// Rebuild the original raw requests against a fresh complete BEFORE image.
    /// Native admission spends `native`; immutable image comparisons and original
    /// intent binding spend `proof`. Both full images and the removed-root count
    /// must reproduce the reviewed candidate before it can be published.
    /// This creates no journal and evaluates no query or stored SQL.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.delete.requalify").entered();
        equal_image(fresh.as_bytes(), self.plan.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let plan = fresh.plan_ordinary_row_deletes(
            self.intent.table(),
            self.intent.deletes(),
            native,
            cancelled,
        )?;
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

#[cfg(test)]
mod tests;
