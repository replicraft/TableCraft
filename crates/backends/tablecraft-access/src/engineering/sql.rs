//! One explicit local SQL UPDATE over the original engineering snapshot.
use super::{
    EngineeringCopy,
    editor::{AccessEngineeringEditor, ScalarUpdateMode},
};
use crate::editing::NativeEditError;
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{QueryError, UpdateError, UpdatePlan};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

/// Resolve only the closed local single-target syntax before a host opens files.
/// This does not bind identifiers, inspect rows or authorize source metadata.
/// Named/external/linked execution and additional SQL actions are not introduced.
pub fn local_update_table(query: &Query) -> Result<&str, QueryError> {
    if !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let ([TableSource::Table(table)], [_]) =
        (update.sources.as_slice(), update.assignments.as_slice())
    else {
        return Err(QueryError::Unsupported);
    };
    if table.database.is_some() {
        return Err(QueryError::Unsupported);
    }
    Ok(table.name.as_str())
}

/// Evaluate all original-row matches and replacements before one typed native
/// engineering finish. Planning never publishes files, invokes active content,
/// converts to a qualified save or retries a refused field under another mode.
pub fn preview_update(
    image: NativeImage,
    query: &Query,
    mode: ScalarUpdateMode,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<EngineeringCopy>, UpdateError<NativeEditError>> {
    let table = local_update_table(query)?;
    let editor = AccessEngineeringEditor::new(image, table, mode, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    tablecraft_query::plan_update(editor, query, budget, collation, cancelled)
}
