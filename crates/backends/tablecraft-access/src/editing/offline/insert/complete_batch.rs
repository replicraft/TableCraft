//! One complete batch retains the same exclusive session and apply lifecycle.
use super::{Cause, OfflineTableInserter, Phase, StagedInsert};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::CompleteInsertableBatch;

impl CompleteInsertableBatch for OfflineTableInserter {
    fn plan_insert_rows(
        self,
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.offline.plan_complete_insert_batch").entered();
        let Self { session, inserter } = self;
        match inserter.plan_insert_rows(rows, budget, cancelled) {
            Ok(plan) => Ok(StagedInsert { session, plan }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}
