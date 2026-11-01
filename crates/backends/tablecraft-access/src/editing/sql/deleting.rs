//! Normal DELETE lowering consumes the existing shared predicate service.
use crate::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
};
use tablecraft_ace::{NativeImage, RecoverableNativeDeletePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{DeleteError, DeletePlan, QueryError};
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
pub fn local_delete_table(query: &Query) -> Result<&str, QueryError> {
    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [TableSource::Table(table)] = delete.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    let qualifier = table.alias.as_ref().unwrap_or(&table.name).as_str();
    if !query.parameters.is_empty()
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
    let table = local_delete_table(query)?;
    let editor = NativeJournalEditor::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_delete(
        editor, query, budget, collation, cancelled,
    )?)
}
