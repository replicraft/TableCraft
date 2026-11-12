//! Normal DELETE lowering consumes the existing shared predicate service.
mod joined;
mod read_source;
use super::{NativeMutationOptions, comparison};
use crate::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
};
pub use joined::{JoinedDeleteTarget, local_joined_delete_target, prepare_native_joined_delete};
pub use read_source::{delete_uses_read_source, prepare_native_read_source_delete};
use tablecraft_ace::{NativeImage, RecoverableNativeDeletePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::DeletableTable;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{DeleteError, DeletePlan, MutationOptions, QueryError};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};
use thiserror::Error;

/// Pre-effect shape, snapshot or normal DELETE planning failure.
#[derive(Debug, Error)]
pub enum RecoverableDeleteError {
    /// Not an admitted explicit local single-table DELETE statement.
    #[error(transparent)]
    Shape(#[from] QueryError),
    /// Native source snapshot preparation failed.
    #[error(transparent)]
    Editor(#[from] NativeEditError),
    /// Predicate evaluation or normal deletion preparation failed.
    #[error(transparent)]
    Plan(#[from] DeleteError<journal::Error>),
}

/// Check the local action before opening a target or reading native records.
pub fn local_delete_table(query: &Query, allow_parameters: bool) -> Result<&str, QueryError> {
    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [TableSource::Table(table)] = delete.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    let qualifier = table.alias.as_ref().unwrap_or(&table.name).as_str();
    if (!allow_parameters && !query.parameters.is_empty())
        || table.database.is_some()
        || (!delete.targets.is_empty()
            && (delete.targets.len() != 1
                || !delete.targets[0].as_str().eq_ignore_ascii_case(qualifier)))
    {
        return Err(QueryError::Unsupported);
    }
    Ok(table.name.as_str())
}

/// Evaluate all original predicates, then qualify one normal source-bound DELETE.
pub fn plan_recoverable_delete(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<RecoverableNativeDeletePlan>, RecoverableDeleteError> {
    let table = local_delete_table(query, false)?;
    let editor = NativeJournalEditor::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_delete(
        editor, query, budget, collation, cancelled,
    )?)
}

/// Evaluate typed parameters and predicates under actual native field policies,
/// keeping native snapshot/admission work separate from query selection work.
/// The result remains a sealed candidate, not permission to publish a journal.
pub fn plan_recoverable_delete_with_options(
    image: NativeImage,
    query: &Query,
    native: &mut ResourceBudget,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<RecoverableNativeDeletePlan>, RecoverableDeleteError> {
    let table = local_delete_table(query, true)?;
    let schema = comparison::schema(&image, table, native, cancelled)?;
    let editor = NativeJournalEditor::new(image, table, native, cancelled)?;
    let policies = comparison::align(&schema, editor.snapshot().columns(), native, cancelled)?;
    Ok(tablecraft_query::plan_delete_with_options(
        editor,
        query,
        query_budget,
        native,
        MutationOptions {
            number_format: options.number_format,
            date_format: options.date_format,
            parameters: Some(options.parameters),
            collation: Some(options.collation),
            collation_resolver: Some(options.resolver),
            like_mode: options.like_mode,
            text_collations: &policies,
        },
        cancelled,
    )?)
}

#[cfg(test)]
mod tests;
