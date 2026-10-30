//! All original predicate results precede a single consuming deletion finish.
use crate::{
    ExecutionOptions, QueryError,
    binding::{BoundDelete, bind_delete},
    discovery::DiscoveryState,
    execution::{Environment, Runtime, cell::CellValue, editing::source::SnapshotSource},
    profile::MAX_BIND_BYTES,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{DeletableTable, RowHandle};
use tablecraft_expressions::{comparison::Collation, logic::Truth};
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Typed predicate/planning failures without SQL, identifiers or row payloads.
#[derive(Debug, Error)]
pub enum DeleteError<E: std::error::Error> {
    /// Binding, expression, allocation, work or cancellation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Backend rejected the complete deletion selection without durable writes.
    #[error("delete backend rejected the candidate: {0}")]
    Backend(#[source] E),
}

/// One backend deletion candidate after every original-row predicate succeeds.
pub struct DeletePlan<P> {
    backend: P,
    matched_rows: usize,
}

impl<P> DeletePlan<P> {
    /// Original WHERE-matching rows, excluding additional cascade deletions.
    pub const fn matched_rows(&self) -> usize {
        self.matched_rows
    }

    /// Consume the wrapper; persistence remains an explicit host responsibility.
    pub fn into_backend(self) -> P {
        self.backend
    }
}

/// Select original rows of one local table, then prepare one deletion candidate.
/// Pure predicates retain the evaluator's eager errors and explicit collation.
/// Joins, parameters, external sources, calls and hidden retrieval remain closed.
/// Even no matches call the backend once, preserving empty-operation admission.
pub fn plan_delete<E: DeletableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
    let _span = tracing::debug_span!("query.delete.plan").entered();
    if cancelled() {
        return Err(QueryError::Cancelled.into());
    }

    budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    let snapshot = editor.snapshot();
    if snapshot.policy().names != NameComparison::AsciiInsensitive {
        return Err(QueryError::Unsupported.into());
    }

    budget
        .charge(Resource::Bytes, MAX_BIND_BYTES as u64)
        .map_err(QueryError::from)?;
    let source = SnapshotSource(snapshot);
    let BoundDelete { filter } = bind_delete(query, &source)?;

    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        source: &source,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options: ExecutionOptions {
            collation,
            cancelled: Some(cancelled),
            ..ExecutionOptions::default()
        },
        budget,
    };
    runtime.tick()?;
    let mut selected: Vec<RowHandle> = Vec::new();

    for (ordinal, row) in snapshot.rows().iter().enumerate() {
        runtime.tick()?;
        runtime
            .budget
            .charge(Resource::Rows, 1)
            .map_err(QueryError::from)?;
        let mut expression_row = runtime.allocate_expression_row(row.len())?;
        for value in row {
            expression_row.push(CellValue::scalar(runtime.copy(value)?));
        }

        let environment = Environment {
            aggregate_results: None,
            collections: None,
            row: &expression_row,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &filter {
            let value = runtime.eval(filter, environment)?;
            if !Truth::from_value(&value)
                .map_err(QueryError::from)?
                .is_true()
            {
                continue;
            }
        }

        let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
        if selected.len() == selected.capacity() {
            let added = selected.capacity().max(1);
            runtime
                .budget
                .charge(Resource::Steps, selected.len() as u64)
                .map_err(QueryError::from)?;
            runtime
                .budget
                .charge_allocation(added as u64, size_of::<RowHandle>() as u64)
                .map_err(QueryError::from)?;
            selected
                .try_reserve_exact(added)
                .map_err(|_| QueryError::Allocation)?;
        }

        selected.push(handle);
    }

    runtime.tick()?;
    let matched_rows = selected.len();
    let backend = editor
        .plan_deletes(&selected, budget, cancelled)
        .map_err(DeleteError::Backend)?;

    Ok(DeletePlan {
        backend,
        matched_rows,
    })
}

#[cfg(test)]
mod tests;
