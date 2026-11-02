//! Normal insertion planning and the existing fixed-length durable prepare path.
use super::NativeClockInsertIntent;
use crate::NativeOmittedInsertIntent;
use crate::transaction::intent::RecoverableEditError;
use crate::{
    EditError, NativeEditPlan, NativeImage, OmittedRowInsert, PreparedNativeEdit, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_io::journal::{DurableStorage, JournalError};

/// A qualified normal INSERT plan retaining the exact original presence rows.
/// There is no public constructor from journal bytes or engineering candidates.
pub struct RecoverableNativeClockInsertPlan {
    plan: NativeEditPlan,
    intent: NativeClockInsertIntent,
}

impl NativeImage {
    /// Qualify native target-local absence and reconstruct original presence/default insertion.
    /// The retained intent contains requests, not final patches or permissions.
    pub fn plan_recoverable_ordinary_clock_row_inserts(
        self,
        table: &str,
        rows: Vec<OmittedRowInsert>,
        provider: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeClockInsertPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_clock_insert.plan_recoverable").entered();
        let original =
            NativeOmittedInsertIntent::new(table, rows, self.as_bytes(), budget, cancelled)?;
        super::codec::length(original.encoded_length(budget, cancelled)?)?;
        // Original row/name/binding/envelope errors precede any needed callback.
        let (plan, reading) = self.capture_ordinary_clock_row_inserts(
            table,
            original.rows(),
            provider,
            budget,
            cancelled,
        )?;
        let intent = NativeClockInsertIntent::from_original(original, reading, budget, cancelled)?;
        if plan.before_bytes().len() != plan.after_bytes().len()
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeClockInsertPlan { plan, intent })
    }
}

impl RecoverableNativeClockInsertPlan {
    /// Original presence request record bound to this plan's complete BEFORE image.
    pub fn intent(&self) -> &NativeClockInsertIntent {
        &self.intent
    }

    /// Complete qualified original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete qualified replacement bytes, not a durable completion receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Number of explicitly requested rows added by this plan.
    pub fn inserted_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Prepare the existing V1 expected-image journal using caller-owned storage.
    /// Hosts retain namespace, exclusivity and interrupted-recovery obligations.
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
