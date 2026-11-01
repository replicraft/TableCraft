//! One original binding chooses exactly one physical planner and journal family.
use super::NativeEofInsertIntent;
use crate::{
    EditError, NativeEditPlan, NativeImage, PreparedResizableNativeEdit,
    RecoverableNativeInsertPlan, RowInsert,
    format::page::PAGE_BYTES,
    header::check,
    table::OrdinaryInsertPlanKind,
    transaction::intent::{
        NativeInsertIntent, RecoverableEditError, complete_insert::CompleteRows,
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};

/// A growing insertion plan retaining closed complete-row TCNA3 data.
pub struct RecoverableNativeEofInsertPlan {
    plan: NativeEditPlan,
    intent: NativeEofInsertIntent,
}

/// One preselected complete-row operation; no failed planner chooses a variant.
pub enum RecoverableNativeInsertOperation {
    /// Existing DATA/index capacity with the unchanged TCNA1/V1 contract.
    Existing(RecoverableNativeInsertPlan),
    /// Exactly one represented new DATA page, TCNA3/V2 only.
    AllocateEof(RecoverableNativeEofInsertPlan),
}

impl NativeImage {
    /// Explicit growing operation; empty/fitting requests refuse unused EOF.
    pub fn plan_recoverable_ordinary_eof_row_inserts(
        self,
        table: &str,
        rows: Vec<RowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEofInsertPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_insert.eof.recoverable").entered();
        let intent = NativeEofInsertIntent::new(table, rows, self.as_bytes(), budget, cancelled)?;
        let plan = self.plan_ordinary_eof_row_inserts(table, intent.rows(), budget, cancelled)?;
        RecoverableNativeEofInsertPlan::from_parts(plan, intent, budget, cancelled)
    }

    /// Complete original rows select existing or represented EOF capacity before
    /// any page staging. Omission/default generation is never substituted here.
    pub fn plan_recoverable_ordinary_auto_row_inserts(
        self,
        table: &str,
        rows: Vec<RowInsert>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeInsertOperation, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_insert.auto.recoverable").entered();
        let original = CompleteRows::new(table, rows, self.as_bytes(), budget, cancelled)?;
        let kind = self.ordinary_insert_plan_kind(table, original.rows(), budget, cancelled)?;
        match kind {
            OrdinaryInsertPlanKind::Existing => {
                let plan =
                    self.plan_ordinary_row_inserts(table, original.rows(), budget, cancelled)?;
                let intent = NativeInsertIntent::from_original(original);
                RecoverableNativeInsertPlan::from_parts(plan, intent, budget, cancelled)
                    .map(RecoverableNativeInsertOperation::Existing)
            }
            OrdinaryInsertPlanKind::AllocateEof => {
                let plan =
                    self.plan_ordinary_eof_row_inserts(table, original.rows(), budget, cancelled)?;
                let intent = NativeEofInsertIntent::from_original(original);
                RecoverableNativeEofInsertPlan::from_parts(plan, intent, budget, cancelled)
                    .map(RecoverableNativeInsertOperation::AllocateEof)
            }
        }
    }
}

impl RecoverableNativeEofInsertPlan {
    fn from_parts(
        plan: NativeEditPlan,
        intent: NativeEofInsertIntent,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RecoverableEditError> {
        if plan.before_bytes().len().checked_add(PAGE_BYTES) != Some(plan.after_bytes().len())
            || plan.changed_rows() != intent.rows().len()
        {
            return Err(EditError::Preservation.into());
        }
        check(budget, cancelled).map_err(EditError::from)?;
        Ok(Self { plan, intent })
    }

    /// Exact original request data, without map cells or absence authority.
    pub fn intent(&self) -> &NativeEofInsertIntent {
        &self.intent
    }

    /// Complete immutable original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Independently proved exact original length plus one DATA page.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Requested cardinality increase, without a matched-old-row count.
    pub fn inserted_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Prepare V2 only with an explicitly resizable target and same journal.
    pub fn prepare_insert<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        self.plan
            .prepare_resizable(target, journal, budget, cancelled)
    }
}
