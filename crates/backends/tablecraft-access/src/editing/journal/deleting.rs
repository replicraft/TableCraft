//! Normal deletion shares the real journal editor's snapshot and source image.
use super::{Error, NativeJournalEditor};
use tablecraft_ace::RecoverableNativeDeletePlan;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, RowHandle, Snapshot};

impl DeletableTable for NativeJournalEditor {
    type Error = Error;
    type Plan = RecoverableNativeDeletePlan;

    fn snapshot(&self) -> &Snapshot {
        EditableTable::snapshot(&self.editor)
    }

    fn plan_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.editor
            .plan_recoverable_deletes(rows, budget, cancelled)
    }
}
