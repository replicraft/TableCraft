//! Snapshot-bound deletion preparation, separate from scalar assignment.
use crate::{RowHandle, Snapshot};
use tablecraft_core::limits::ResourceBudget;

/// One immutable table and one consuming, nonpersistent deletion preparation.
pub trait DeletableTable: Sized {
    /// Backend error with its typed cause retained.
    type Error: std::error::Error;
    /// Backend-owned candidate, not permission to commit or publish it.
    type Plan;

    /// Original rows which issue every accepted deletion handle.
    fn snapshot(&self) -> &Snapshot;

    /// Prepare the complete selected handle set once. Reject stale or repeated
    /// handles. An empty set still requires the backend's normal source gates.
    /// Failure publishes no partial candidate and performs no durable writes.
    fn plan_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error>;
}
