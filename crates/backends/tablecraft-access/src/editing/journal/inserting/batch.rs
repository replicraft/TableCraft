//! One atomic TCNA2 action retains every original row and presence bit.
use super::NativeJournalInserter;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{InsertValue, OmittingInsertableBatch};

impl OmittingInsertableBatch for NativeJournalInserter {
    fn plan_insert_rows_with_omissions(
        self,
        rows: Vec<Vec<InsertValue>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.plan_presence_rows(rows, None, budget, cancelled)
    }
}
