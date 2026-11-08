//! Normal INSERT retains complete values or explicit original field presence.
mod select;
use super::NativeMutationOptions;
use crate::editing::{
    NativeEditError,
    journal::{self, NativeJournalInsertPlan, NativeJournalInserter},
};
pub use select::{
    PreparedNativeInsertSelect, local_insert_select_table, prepare_native_insert_select,
};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::OmittingInsertableTable;
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_query::{InsertError, QueryError};
use tablecraft_sql::query::{Query, Statement};
use thiserror::Error;

/// Failure before any journal or target mutation exists.
#[derive(Debug, Error)]
pub enum RecoverableInsertError {
    /// Not an admitted local INSERT source profile.
    #[error(transparent)]
    Shape(#[from] QueryError),
    /// Actual native metadata inspection failed.
    #[error(transparent)]
    Editor(#[from] NativeEditError),
    /// Expression binding, exact carriers or native INSERT admission refused.
    #[error(transparent)]
    Plan(#[from] InsertError<journal::Error>),
}

/// Reject external sources, SELECT and unsupported shape before open.
/// Parameter syntax requires explicit admission; typed values are checked later.
pub fn local_insert_table<'a>(
    query: &'a Query,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a str, QueryError> {
    tablecraft_query::local_insert_table(query, false, allow_parameters, budget, cancelled)
}

/// Bind supplied expressions once and retain omitted fields through native finish.
pub fn plan_recoverable_insert(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError> {
    let table = local_insert_table(query, false, budget, cancelled)?;
    let inserter = NativeJournalInserter::new(image, table, budget, cancelled)?;
    plan_bound(inserter, query, budget, cancelled)
}

/// Use an explicitly supplied host clock only for source-qualified needed defaults.
/// Metadata capture and complete supplied requests never sample the provider.
pub fn plan_recoverable_insert_with_clock(
    image: NativeImage,
    query: &Query,
    provider: &dyn Clock,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError> {
    let table = local_insert_table(query, false, budget, cancelled)?;
    let inserter =
        NativeJournalInserter::new(image, table, budget, cancelled)?.with_clock(provider);
    plan_bound(inserter, query, budget, cancelled)
}

fn plan_bound<I>(
    inserter: I,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError>
where
    I: OmittingInsertableTable<Error = journal::Error, Plan = NativeJournalInsertPlan>,
{
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported.into());
    };
    // Choose original presence structurally from the written field list and
    // actual metadata, before expressions or native planning. No failed plan
    // is retried under another default/complete-row policy.
    let omitted = !insert.fields.is_empty() && insert.fields.len() < inserter.columns().len();
    if omitted {
        Ok(tablecraft_query::plan_insert_with_omissions(
            inserter, query, budget, cancelled,
        )?)
    } else {
        Ok(tablecraft_query::plan_insert(
            inserter, query, budget, cancelled,
        )?)
    }
}

/// Review explicit typed VALUES with an independent query allowance. The optional
/// clock authorizes only source-qualified needed omitted defaults, never SQL calls.
/// Complete and omission intent are selected structurally before evaluation.
pub fn plan_recoverable_insert_with_options(
    image: NativeImage,
    query: &Query,
    native: &mut ResourceBudget,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    clock: Option<&dyn Clock>,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError> {
    let table = local_insert_table(query, true, query_budget, cancelled)?;
    let inserter = NativeJournalInserter::new(image, table, native, cancelled)?;
    match clock {
        Some(provider) => plan_bound_with_options(
            inserter.with_clock(provider),
            query,
            native,
            query_budget,
            options,
            cancelled,
        ),
        None => plan_bound_with_options(inserter, query, native, query_budget, options, cancelled),
    }
}

fn plan_bound_with_options<I>(
    inserter: I,
    query: &Query,
    native: &mut ResourceBudget,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError>
where
    I: OmittingInsertableTable<Error = journal::Error, Plan = NativeJournalInsertPlan>,
{
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported.into());
    };
    let omitted = !insert.fields.is_empty() && insert.fields.len() < inserter.columns().len();
    let options = tablecraft_query::MutationOptions {
        parameters: Some(options.parameters),
        collation: Some(options.collation),
        collation_resolver: Some(options.resolver),
        text_collations: &[],
    };
    if omitted {
        Ok(tablecraft_query::plan_insert_with_omissions_and_options(
            inserter,
            query,
            query_budget,
            native,
            options,
            cancelled,
        )?)
    } else {
        Ok(tablecraft_query::plan_insert_with_options(
            inserter,
            query,
            query_budget,
            native,
            options,
            cancelled,
        )?)
    }
}

#[cfg(test)]
mod tests;
