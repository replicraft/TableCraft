//! Ordered complete rows retain one backend preparation and atomic apply boundary.
use super::InsertableTable;
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Complete typed row batches with no omitted-field or generation authority.
/// Each row uses the target's complete supplied-column order. Implementations
/// must validate the entire batch before publishing a plan; no row is committed
/// independently. The backend retains its own bounds, constraints and exact
/// carrier policy. SQL syntax and durable storage remain outside this contract.
pub trait CompleteInsertableBatch: InsertableTable {
    /// Consume the original ordered complete rows in one nonpersistent finish.
    /// A failure publishes no partial candidate or durable effects. Empty-batch
    /// semantics remain the selected backend's explicitly checked policy.
    fn plan_insert_rows(
        self,
        rows: Vec<Vec<Value>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error>;
}
