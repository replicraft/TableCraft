use super::{NativeInt32EditIntent, NativeIntentError};
use crate::{EditError, Int32Update, NativeEditPlan, NativeImage, PreparedNativeEdit};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError};
use thiserror::Error;

/// Existing native qualification or additional intent-binding failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoverableEditError {
    /// The unchanged normal native planner refused the operation.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// Original request metadata could not be validated/copied/bound.
    #[error(transparent)]
    Intent(#[from] NativeIntentError),
}

/// A qualified normal Jet4 plan retaining its original resolved request intent.
/// No public constructor from parts or decoded request grants this authority.
pub struct RecoverableNativeEditPlan {
    plan: NativeEditPlan,
    intent: NativeInt32EditIntent,
}

impl NativeImage {
    /// Plan the unchanged normal Jet4 Int32 profile, then bind moved original
    /// requests to its exact before image. Old planning APIs/costs are unchanged.
    /// Intent failure occurs before any journal/target write and returns no plan.
    pub fn plan_recoverable_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: Vec<Int32Update>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.edit.plan_recoverable").entered();
        let plan = self.plan_int32_updates(table, column, &updates, budget, cancelled)?;
        let intent = NativeInt32EditIntent::new(
            table,
            column,
            updates,
            plan.before_bytes(),
            budget,
            cancelled,
        )?;
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}

impl RecoverableNativeEditPlan {
    /// Original immutable resolved requests and source binding.
    pub fn intent(&self) -> &NativeInt32EditIntent {
        &self.intent
    }

    /// Complete qualified original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete qualified replacement bytes, not a durable-save receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Effective changed rows; no-op requests remain in the intent separately.
    pub fn changed_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Delegate existing expected-image journal preparation. Hosts still own
    /// correct distinct pairing, durable namespace setup and retained resources.
    /// Dropping/interruption retains the journal; every commit error needs recovery.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        self.plan.prepare(target, journal, budget, cancelled)
    }
}
