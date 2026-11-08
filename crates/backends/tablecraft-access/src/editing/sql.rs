//! Shared normal UPDATE planning; file ownership and publication stay with hosts.
#[cfg(feature = "journal")]
mod comparison;
#[cfg(feature = "journal")]
mod deleting;
#[cfg(feature = "journal")]
mod inserting;
#[cfg(feature = "journal")]
pub use deleting::{RecoverableDeleteError, local_delete_table, plan_recoverable_delete};
#[cfg(feature = "journal")]
pub use inserting::{
    RecoverableInsertError, local_insert_table, plan_recoverable_insert,
    plan_recoverable_insert_with_clock,
};

#[cfg(feature = "journal")]
use super::journal::{self, NativeJournalEditor};
use super::{NativeEditError, NativeEditor};
#[cfg(feature = "journal")]
use tablecraft_ace::RecoverableNativeEditPlan;
use tablecraft_ace::{NativeEditPlan, NativeImage};
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "journal")]
use tablecraft_core::values::Value;
#[cfg(feature = "journal")]
use tablecraft_db::EditableTable;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{QueryError, UpdateError, UpdatePlan};
#[cfg(feature = "journal")]
use tablecraft_query::{UpdateOptions, source::CollationResolver};
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

/// Failures of the explicit recoverable planning path, before durable writes.
#[cfg(feature = "journal")]
#[derive(Debug, Error)]
pub enum RecoverableError {
    /// The statement is outside the local single-table UPDATE profile.
    #[error(transparent)]
    Shape(#[from] QueryError),
    /// Native snapshot preparation failed.
    #[error(transparent)]
    Editor(#[from] NativeEditError),
    /// Evaluation, native validation or intent construction failed.
    #[error(transparent)]
    Plan(#[from] UpdateError<journal::Error>),
}

/// Resolve SQL once against the original snapshot and retain the typed native
/// request for recovery qualification. No SQL or evaluator state is persisted;
/// journal creation and commit remain explicit host operations.
#[cfg(feature = "journal")]
pub fn plan_recoverable_update(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<RecoverableNativeEditPlan>, RecoverableError> {
    let table = local_update_table(query)?;
    let editor = NativeJournalEditor::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_update(
        editor, query, budget, collation, cancelled,
    )?)
}

/// Check a closed local UPDATE with one or more original-snapshot SET targets.
/// Syntax admission does not grant native program or fixed-storage permission.
/// Parameter declarations require explicit caller admission; typed values are
/// separately validated by the consuming planner.
#[cfg(feature = "journal")]
pub fn local_batch_update_table(query: &Query, allow_parameters: bool) -> Result<&str, QueryError> {
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [TableSource::Table(table)] = update.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    if (!allow_parameters && !query.parameters.is_empty())
        || table.database.is_some()
        || update.assignments.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    Ok(table.name.as_str())
}

/// Evaluate every SET expression against the original captured snapshot, then
/// finish one normal batch. Recovery retains resolved carriers, never SQL text.
#[cfg(feature = "journal")]
pub fn plan_recoverable_batch_update(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<RecoverableNativeEditPlan>, RecoverableError> {
    let table = local_batch_update_table(query, false)?;
    let editor = NativeJournalEditor::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_batch_update(
        editor, query, budget, collation, cancelled,
    )?)
}

/// Evaluate the closed local batch using an independent query allowance.
/// Snapshot construction and the single native finish, including original
/// intent binding, consume the native allowance. No journal or lease is created.
#[cfg(feature = "journal")]
pub fn plan_recoverable_batch_update_with_budgets(
    image: NativeImage,
    query: &Query,
    native: &mut ResourceBudget,
    query_budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<RecoverableNativeEditPlan>, RecoverableError> {
    let table = local_batch_update_table(query, false)?;
    let editor = NativeJournalEditor::new(image, table, native, cancelled)?;
    Ok(tablecraft_query::plan_batch_update_with_budgets(
        editor,
        query,
        query_budget,
        native,
        collation,
        cancelled,
    )?)
}

/// Explicit evaluation inputs for one native original-snapshot UPDATE.
#[cfg(feature = "journal")]
pub struct NativeUpdateOptions<'a> {
    /// Complete typed literal inputs, validated against the SQL declarations.
    pub parameters: &'a [(&'a str, Value)],
    /// Database comparison provider for expressions without stored-field policy.
    pub collation: &'a dyn Collation,
    /// Resolve the actual native stored-field comparison identities.
    pub resolver: &'a dyn CollationResolver,
}

/// Plan one recoverable local UPDATE with actual native field text policies.
/// Database/default and field comparison providers remain explicit. Native
/// declaration capture and policy alignment consume `native`; query evaluation
/// consumes `query_budget`. No field policy enters the neutral DB snapshot.
#[cfg(feature = "journal")]
pub fn plan_recoverable_batch_update_with_options(
    image: NativeImage,
    query: &Query,
    native: &mut ResourceBudget,
    query_budget: &mut ResourceBudget,
    options: NativeUpdateOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<RecoverableNativeEditPlan>, RecoverableError> {
    let table = local_batch_update_table(query, true)?;
    let schema = comparison::schema(&image, table, native, cancelled)?;
    let editor = NativeJournalEditor::new(image, table, native, cancelled)?;
    let policies = comparison::align(&schema, editor.snapshot().columns(), native, cancelled)?;
    let comparison = UpdateOptions {
        parameters: Some(options.parameters),
        collation: Some(options.collation),
        collation_resolver: Some(options.resolver),
        text_collations: &policies,
    };

    Ok(tablecraft_query::plan_batch_update_with_comparison(
        editor,
        query,
        query_budget,
        native,
        comparison,
        cancelled,
    )?)
}

#[cfg(all(test, feature = "journal"))]
mod budgets;
