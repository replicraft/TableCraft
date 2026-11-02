//! Ordered rows retain the existing exclusive session and one apply lifecycle.
use super::{Cause, OfflineTableInserter, Phase, StagedInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{InsertValue, OmittingInsertableBatch};

impl OmittingInsertableBatch for OfflineTableInserter {
    fn plan_insert_rows_with_omissions(
        self,
        rows: Vec<Vec<InsertValue>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.offline.plan_omitted_insert_batch").entered();
        let Self { session, inserter } = self;

        match inserter.plan_insert_rows_with_omissions(rows, budget, cancelled) {
            Ok(plan) => Ok(StagedInsert { session, plan }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}
