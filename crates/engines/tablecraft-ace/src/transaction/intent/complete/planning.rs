//! One complete original request dialect with source-derived durable geometry.
mod requalify;
use super::NativeCompleteInsertIntent;
use crate::{
    EditError, NativeEditPlan, NativeImage, PreparedNativeEdit, PreparedNativeUpdate, RowInsert,
    format::page::PAGE_BYTES,
    header::check,
    limits::MAX_IMAGE_BYTES,
    transaction::intent::{RecoverableEditError, complete_insert::CompleteRows},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

/// A complete original TCNA4 request and its independently checked full plan.
/// Storage geometry is derived by native planning and is not encoded permission.
pub struct RecoverableNativeCompleteInsertPlan {
    plan: NativeEditPlan,
    intent: NativeCompleteInsertIntent,
}

impl NativeImage {
    /// Plan exactly the original complete rows, including empty batches and NULL.
    /// Defaults/omission have their own dialect and are never substituted here.
    pub fn plan_recoverable_ordinary_complete_row_inserts(
        self,
        table: &str,
        rows: Vec<RowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeCompleteInsertPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary.complete_insert.recoverable").entered();
        let original = CompleteRows::new_complete(table, rows, self.as_bytes(), budget, cancelled)?;
        let intent = NativeCompleteInsertIntent::from_original(original);
        let plan =
            self.plan_ordinary_complete_row_inserts(table, intent.rows(), budget, cancelled)?;
        let before = plan.before_bytes().len();
        let after = plan.after_bytes().len();
        if after < before
            || !(after - before).is_multiple_of(PAGE_BYTES)
            || after as u64 > MAX_IMAGE_BYTES
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(RecoverableNativeCompleteInsertPlan { plan, intent })
    }
}

impl RecoverableNativeCompleteInsertPlan {
    /// Exact original rows, not expanded defaults or final placement decisions.
    pub fn intent(&self) -> &NativeCompleteInsertIntent {
        &self.intent
    }

    /// Complete original native image.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete independently proved replacement, with no unused EOF pages.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Requested cardinality increase, without a matched-old-row count.
    pub fn inserted_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Fixed-only storage preserves the explicit no-resize boundary.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        if self.before_bytes().len() != self.after_bytes().len() {
            return Err(JournalError::ImageSize);
        }
        self.plan.prepare(target, journal, budget, cancelled)
    }

    /// Choose V1 or V2 once from the already proved native plan's exact lengths.
    pub fn prepare_insert<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeUpdate<'a, D, J>, JournalError> {
        if self.before_bytes().len() == self.after_bytes().len() {
            self.plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed)
        } else {
            self.plan
                .prepare_resizable(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Resizable)
        }
    }
}
