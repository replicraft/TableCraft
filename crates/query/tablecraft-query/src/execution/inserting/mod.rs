//! One VALUES row: complete or explicit omission intent, with one backend finish.
use crate::{QueryError, execution::editing::assignment::stored_value};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
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
    let staged = values::prepare(&editor, query, false, budget, cancelled)?;
    let width = staged.len();

    budget
        .charge_allocation(width as u64, size_of::<Value>() as u64)
        .map_err(QueryError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(width)
        .map_err(|_| QueryError::Allocation)?;
    for value in staged {
        values::check(budget, cancelled)?;
        values.push(value.ok_or(QueryError::Width)?);
    }

    budget.charge(Resource::Rows, 1).map_err(QueryError::from)?;
    values::check(budget, cancelled)?;

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
    let staged = values::prepare(&editor, query, true, budget, cancelled)?;

    budget
        .charge_allocation(staged.len() as u64, size_of::<InsertValue>() as u64)
        .map_err(QueryError::from)?;
    let mut intent = Vec::new();
    intent
        .try_reserve_exact(staged.len())
        .map_err(|_| QueryError::Allocation)?;
    for value in staged {
        values::check(budget, cancelled)?;
        intent.push(match value {
            Some(value) => InsertValue::Supplied(value),
            None => InsertValue::Omitted,
        });
    }

    budget.charge(Resource::Rows, 1).map_err(QueryError::from)?;
    values::check(budget, cancelled)?;
    editor
        .plan_insert_with_omissions(intent, budget, cancelled)
        .map_err(InsertError::Backend)
}
