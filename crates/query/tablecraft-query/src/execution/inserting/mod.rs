//! One VALUES row: complete or explicit omission intent, with one backend finish.
use crate::{MutationOptions, QueryError, execution::editing::assignment::stored_value};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{InsertableTable, OmittingInsertableTable};
use tablecraft_sql::query::Query;
use thiserror::Error;

#[cfg(test)]
mod omissions_tests;
mod shape;
#[cfg(test)]
mod tests;
mod values;

pub use shape::local_insert_table;

/// Query binding/evaluation failure or one complete backend INSERT refusal.
#[derive(Debug, Error)]
pub enum InsertError<E: std::error::Error> {
    /// Sanitized syntax, metadata, scalar, resource or cancellation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// The backend refused the complete row intent without durable writes.
    #[error("insert backend rejected the candidate: {0}")]
    Backend(#[source] E),
}

/// Evaluate one complete local VALUES row, then invoke one backend preparation.
/// Explicit fields may reorder all supplied columns but cannot omit, repeat or
/// invent a field. Scalar expressions evaluate in written VALUES order; the
/// resulting row is assembled in backend schema order. No original-row scan,
/// default substitution, normal save or source-opening capability is introduced.
pub fn plan_insert<E: InsertableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<E::Plan, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.values_plan").entered();
    let staged = values::prepare(&editor, query, false, None, budget, cancelled)?;
    let values = values::complete(staged, budget, cancelled)?;

    editor
        .plan_insert(values, budget, cancelled)
        .map_err(InsertError::Backend)
}

/// Preserve supplied versus omitted positions in one complete-width row intent.
/// Named fields may omit ordinary backend columns; unnamed VALUES remain full
/// width. Explicit NULL remains supplied, and defaults belong to the backend.
pub fn plan_insert_with_omissions<E: OmittingInsertableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<E::Plan, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.omission_plan").entered();
    let staged = values::prepare(&editor, query, true, None, budget, cancelled)?;

    let intent = values::presence(staged, budget, cancelled)?;
    editor
        .plan_insert_with_omissions(intent, budget, cancelled)
        .map_err(InsertError::Backend)
}

/// Evaluate one VALUES row with strict typed inputs and an independent backend
/// allowance. All written expressions succeed before the consuming finish.
pub fn plan_insert_with_options<E: InsertableTable>(
    editor: E,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    options: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<E::Plan, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.values_plan").entered();
    let staged = values::prepare(
        &editor,
        query,
        false,
        Some(options),
        query_budget,
        cancelled,
    )?;
    let output = values::complete(staged, query_budget, cancelled)?;
    editor
        .plan_insert(output, backend_budget, cancelled)
        .map_err(InsertError::Backend)
}

/// Evaluate one VALUES row with strict typed inputs and an independent backend
/// allowance. All written expressions succeed before the consuming finish.
pub fn plan_insert_with_omissions_and_options<E: OmittingInsertableTable>(
    editor: E,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    options: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<E::Plan, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.values_plan").entered();
    let staged = values::prepare(&editor, query, true, Some(options), query_budget, cancelled)?;
    let output = values::presence(staged, query_budget, cancelled)?;
    editor
        .plan_insert_with_omissions(output, backend_budget, cancelled)
        .map_err(InsertError::Backend)
}
