//! Correlated predicates stage updates from only the original editor snapshot.
use super::{
    UpdateError, UpdatePlan,
    plan::{PreparedUpdate, finish_batch, prepare_with_source},
};
use crate::{MutationOptions, QueryError, Source};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{EditableBatch, SnapshotId};

/// Original target identity and explicit comparison/typed-parameter policies.
pub struct ReadSourceUpdateOptions<'a> {
    /// Exact original editor snapshot, including an empty target.
    pub target_snapshot: SnapshotId,
    /// Original target field comparison and parameter policies.
    pub mutation: MutationOptions<'a>,
}

/// Complete original-row WHERE/SET evaluation with no retained Source/pool borrow.
#[must_use = "finish explicitly or discard the staged update"]
pub struct PreparedReadSourceUpdate<E: EditableBatch> {
    editor: E,
    prepared: PreparedUpdate,
}

impl<E: EditableBatch> PreparedReadSourceUpdate<E> {
    /// Original matching target rows, including assignments of unchanged values.
    pub const fn matched_rows(&self) -> usize {
        self.prepared.matched_rows
    }

    /// Qualify exactly once after all predicate read callbacks have ended.
    /// Empty selections still perform complete backend admission.
    ///
    /// # Errors
    /// Retains cancellation/allocation/query errors and the typed backend failure.
    pub fn finish(
        self,
        backend_budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        let columns = self.prepared.borrowed_columns(backend_budget, cancelled)?;
        finish_batch(
            self.editor,
            &self.prepared,
            &columns,
            backend_budget,
            cancelled,
        )
    }
}

/// Evaluate local scalar EXISTS/IN predicates using explicit readonly sources.
/// Outer rows, old values and handles come only from the editor snapshot. SET
/// expressions remain the existing deterministic scalar language and all use the
/// original row, not earlier assignments. Named/external/callable activation,
/// joined targets and scalar/quantified subquery assignments remain closed.
/// No backend budget is borrowed or editor finish invoked during preparation.
///
/// # Errors
/// Refuses snapshot/schema/policy mismatch, unsupported syntax, expression errors,
/// invalid parameters, cancellation and allocation/work exhaustion before finish.
pub fn prepare_read_source_batch_update_with_options<E: EditableBatch>(
    editor: E,
    reads: &dyn Source,
    query: &tablecraft_sql::query::Query,
    query_budget: &mut ResourceBudget,
    options: ReadSourceUpdateOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedReadSourceUpdate<E>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.read_source_prepare").entered();
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
    let prepared = prepare_with_source(
        &editor,
        query,
        true,
        query_budget,
        options.mutation,
        Some(reads),
        cancelled,
    )?;
    Ok(PreparedReadSourceUpdate { editor, prepared })
}

#[cfg(test)]
mod tests;
