//! Correlated SELECT predicates select only the editor's original row handles.
use super::{DeleteError, DeletePlan, finish, select_rows_with_source};
use crate::{MutationOptions, QueryError, Source};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{DeletableTable, RowHandle, SnapshotId};
use tablecraft_sql::query::Query;

/// Frozen target identity and explicit scalar comparison/parameter policies.
/// Read Source stability remains its own contract, not an inferred revision.
pub struct ReadSourceDeleteOptions<'a> {
    /// Exact original editor snapshot, including an empty target.
    pub target_snapshot: SnapshotId,
    /// Typed parameters and comparison policies for the original target.
    pub mutation: MutationOptions<'a>,
}

/// Fully evaluated original-row selection, retaining no read-source or pool borrow.
#[must_use = "finish explicitly or discard the staged deletion"]
pub struct PreparedReadSourceDelete<E: DeletableTable> {
    editor: E,
    selected: Vec<RowHandle>,
}

impl<E: DeletableTable> PreparedReadSourceDelete<E> {
    /// Number of selected original target rows; no cascade count is inferred.
    pub fn matched_rows(&self) -> usize {
        self.selected.len()
    }

    /// Qualify once after read callbacks and their resource borrows have ended.
    /// Empty selections still perform complete backend admission.
    ///
    /// # Errors
    /// Cancellation retains QueryError; backend failures retain their typed cause.
    pub fn finish(
        self,
        backend_budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        finish(self.editor, &self.selected, backend_budget, cancelled)
    }
}

/// Evaluate one local DELETE with explicit scalar EXISTS/IN SELECT read sources.
/// Target rows and identities always come from the editor snapshot. Direct local,
/// ungrouped scalar SELECT subqueries use the existing correlation, NULL and eager
/// evaluation semantics. Named queries, external sources, collection projections,
/// aggregates, scalar/quantified subqueries and callable activation stay closed.
/// No backend budget is borrowed during preparation and no durable effect occurs.
///
/// # Errors
/// Rejects mismatched snapshot/schema/policy, unsupported syntax, invalid parameters,
/// expression errors, cancellation and allocation/work exhaustion before finish.
pub fn prepare_read_source_delete_with_options<E: DeletableTable>(
    editor: E,
    reads: &dyn Source,
    query: &Query,
    query_budget: &mut ResourceBudget,
    options: ReadSourceDeleteOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedReadSourceDelete<E>, DeleteError<E::Error>> {
    let _span = tracing::debug_span!("query.delete.read_source_prepare").entered();
    if cancelled() {
        return Err(QueryError::Cancelled.into());
    }
    query_budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    let snapshot = editor.snapshot();
    if snapshot.id() != options.target_snapshot {
        return Err(QueryError::Source.into());
    }
    query_budget
        .charge(Resource::Steps, snapshot.columns().len() as u64)
        .map_err(QueryError::from)?;
    if reads.columns(snapshot.name())? != snapshot.columns() {
        return Err(QueryError::SchemaChanged.into());
    }
    if !options.mutation.text_collations.is_empty()
        && reads.text_collations(snapshot.name())? != options.mutation.text_collations
    {
        return Err(QueryError::SchemaChanged.into());
    }
    let selected = select_rows_with_source(
        &editor,
        query,
        query_budget,
        options.mutation,
        Some(reads),
        cancelled,
    )?;
    Ok(PreparedReadSourceDelete { editor, selected })
}

#[cfg(test)]
mod tests;
