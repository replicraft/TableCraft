//! Original declaration and every cell reconstruct against one checked V3 BEFORE.
use super::NativePopulatedAddTableIntent;
use crate::{
    NativeImage, NativeRecoveryQualificationError, header::check,
    transaction::qualified_recovery::equal_image,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

/// The same immutable V3 session after complete original native reconstruction.
/// No journal replacement, implicit cleanup or unchecked resizable route exists.
#[must_use = "recover explicitly or retain the qualified populated-table journal"]
pub struct QualifiedPopulatedAddTableRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl NativePopulatedAddTableIntent {
    /// Verify the original binding, qualify actual source roles and reproduce
    /// every AFTER byte before any recovery mutation. Native construction and
    /// original binding/image comparison use distinct caller-owned pools; neither
    /// pool is reset, refilled or substituted. SQL/defaults are never reexecuted.
    ///
    /// # Errors
    /// Rejects stale BEFORE, unsupported source/request, any reconstructed image
    /// mismatch, cancellation, allocation and either caller pool's exhaustion.
    pub fn qualify_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedPopulatedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        self.qualify_with_pools(checked, scope, native, Some(proof), cancelled)
    }

    /// Qualify with one cumulative caller pool for sequential native and proof work.
    /// The pool is never reset or replaced; V3 reconstruction remains identical
    /// to the explicit two-pool entry point.
    pub fn qualify_recovery_shared<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedPopulatedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        self.qualify_with_pools(checked, scope, budget, None, cancelled)
    }

    fn qualify_with_pools<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        native: &mut ResourceBudget,
        mut proof: Option<&mut ResourceBudget>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedPopulatedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.populated_table_recovery.qualify").entered();
        {
            let work = match proof.as_deref_mut() {
                Some(proof) => proof,
                None => &mut *native,
            };
            check(work, cancelled)?;
            self.verify_before(checked.before_bytes(), work, cancelled)?;
        }
        let image = NativeImage::capture_retained(
            &mut Cursor::new(checked.before_bytes()),
            scope,
            native,
            cancelled,
        )?;
        let plan = image.plan_populated_add_table(self.definition, self.rows, native, cancelled)?;
        let work = match proof {
            Some(proof) => proof,
            None => native,
        };
        equal_image(plan.before_bytes(), checked.before_bytes(), work, cancelled)?;
        equal_image(plan.after_bytes(), checked.after_bytes(), work, cancelled)?;
        check(work, cancelled)?;
        Ok(QualifiedPopulatedAddTableRecovery { checked })
    }
}

impl<J: DurableStorage> QualifiedPopulatedAddTableRecovery<'_, J> {
    /// Existing framing state of the same checked journal, not target health.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Restore pending BEFORE or verify committed AFTER through that exact V3
    /// session. The host owns the separate I/O pool and durable cleanup policy.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.populated_table_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Observe an already settled exact image with storage barriers and no writes.
    /// Pending requires BEFORE; committed requires AFTER. Opposite/mixed images
    /// refuse instead of silently replaying or replacing a retained session.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span =
            tracing::debug_span!("native.populated_table_recovery.confirm_completed").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
