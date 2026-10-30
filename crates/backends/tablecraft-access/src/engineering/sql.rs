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
    local_target(query, false)
}

/// Resolve one local UPDATE with one or more SET targets before host opens.
/// Duplicate column identities and all expression binding remain query-owned.
pub fn local_batch_update_table(query: &Query) -> Result<&str, QueryError> {
    local_target(query, true)
}

fn local_target(query: &Query, multiple: bool) -> Result<&str, QueryError> {
    if !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [TableSource::Table(table)] = update.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    if update.assignments.is_empty() || (!multiple && update.assignments.len() != 1) {
        return Err(QueryError::Unsupported);
    }
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

/// Evaluate all SET targets against the original snapshot, then finish once.
/// Fixed modes can combine columns; variable/LVAL modes retain one-column scope.
/// Normal NativeEditor/save admission and the old single-column entry stay strict.
pub fn preview_batch_update(
    image: NativeImage,
    query: &Query,
    mode: ScalarUpdateMode,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<EngineeringCopy>, UpdateError<NativeEditError>> {
    let table = local_batch_update_table(query)?;
    let editor = AccessEngineeringEditor::new(image, table, mode, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    tablecraft_query::plan_batch_update(editor, query, budget, collation, cancelled)
}
