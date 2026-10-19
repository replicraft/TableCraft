use std::sync::{Arc, atomic::AtomicBool};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::QueryLimits;

/// Owned per-request controls suitable for moving to the database worker.
pub struct AsyncOptions {
    /// Cumulative metadata, row, allocation and SQLite VM work allowance.
    pub storage_budget: ResourceBudget,
    /// Separate TableCraft evaluation/materialization ceilings.
    pub query_limits: QueryLimits,
    /// Explicit wildcard convention for TableCraft LIKE evaluation.
    pub like_mode: Option<tablecraft_expressions::predicates::LikeMode>,
    /// Optional explicit text ordering/equality policy, callable on the worker.
    pub collation: Option<Arc<dyn Collation + Send + Sync>>,
    /// Request-specific cancellation flag. Dropping the request future sets it.
    /// Use a fresh flag for each request; cancellation is not automatically reset.
    pub cancelled: Arc<AtomicBool>,
}

impl AsyncOptions {
    /// Supply explicit storage limits and cancellation; use the standard bounded
    /// query limits and no implicit text collation.
    pub fn new(storage_budget: ResourceBudget, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            storage_budget,
            query_limits: QueryLimits::default(),
            collation: None,
            like_mode: None,
            cancelled,
        }
    }
}
