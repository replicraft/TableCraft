//! Closed local DELETE selection over one original engineering snapshot.
use super::{DeleteMode, EngineeringCopy, deleting::AccessEngineeringDeleter};
use crate::NativeEditError;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{DeleteError, DeletePlan, QueryError};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

/// Check local single-target DELETE syntax before a host opens any source.
/// This does not bind fields, scan rows, activate content or authorize a save.
pub fn local_delete_table(query: &Query) -> Result<&str, QueryError> {
    if !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }

    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [TableSource::Table(table)] = delete.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    let qualifier = table.alias.as_ref().unwrap_or(&table.name).as_str();
    if table.database.is_some()
        || (!delete.targets.is_empty()
            && (delete.targets.len() != 1
                || !delete.targets[0].as_str().eq_ignore_ascii_case(qualifier)))
    {
        return Err(QueryError::Unsupported);
    }

    Ok(table.name.as_str())
}

/// Finish all original-row predicate results before one typed native preview.
/// The explicit mode is never inferred, retried or converted to a normal plan.
pub fn preview_delete(
    image: NativeImage,
    query: &Query,
    mode: DeleteMode,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<EngineeringCopy>, DeleteError<NativeEditError>> {
    let table = local_delete_table(query)?;
    let editor = AccessEngineeringDeleter::new(image, table, mode, budget, cancelled)
        .map_err(DeleteError::Backend)?;

    tablecraft_query::plan_delete(editor, query, budget, collation, cancelled)
}
