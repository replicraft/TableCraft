//! Shared normal UPDATE planning; file ownership and publication stay with hosts.

use super::{NativeEditError, NativeEditor};
use tablecraft_ace::{NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{QueryError, UpdateError, UpdatePlan};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};
use thiserror::Error;

/// Failures retain the existing shape, adapter and query-planning boundaries.
#[derive(Debug, Error)]
pub enum Error {
    /// The statement is outside the local single-table UPDATE profile.
    #[error(transparent)]
    Shape(#[from] QueryError),
    /// Native snapshot preparation failed.
    #[error(transparent)]
    Editor(#[from] NativeEditError),
    /// Evaluation or native candidate validation failed.
    #[error(transparent)]
    Plan(#[from] UpdateError<NativeEditError>),
}

/// Check the closed local shape without opening files or inspecting records.
/// External selectors never authorize an edit, including a fresh-copy edit.
pub fn local_update_table(query: &Query) -> Result<&str, QueryError> {
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let ([TableSource::Table(table)], [_]) =
        (update.sources.as_slice(), update.assignments.as_slice())
    else {
        return Err(QueryError::Unsupported);
    };
    if !query.parameters.is_empty() || table.database.is_some() {
        return Err(QueryError::Unsupported);
    }

    Ok(table.name.as_str())
}

/// Prepare one normal native candidate from the original captured image.
/// This composes the existing snapshot adapter and query planner once. It does
/// not publish, journal or grant broader native write eligibility. Hosts supply
/// an explicitly qualified comparison policy and retain file ownership.
pub fn plan_update(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<NativeEditPlan>, Error> {
    let table = local_update_table(query)?;
    let editor = NativeEditor::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_update(
        editor, query, budget, collation, cancelled,
    )?)
}
