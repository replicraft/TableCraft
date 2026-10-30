//! One fully explicit VALUES row, evaluated before one consuming backend finish.
use crate::{
    QueryError, execution::editing::assignment::stored_value, profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertableTable;
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::{InsertSource, Query};
use thiserror::Error;

mod shape;
#[cfg(test)]
mod tests;

pub use shape::local_insert_table;

/// Query binding/evaluation failure or one complete backend INSERT refusal.
#[derive(Debug, Error)]
pub enum InsertError<E: std::error::Error> {
    /// Sanitized syntax, metadata, scalar, resource or cancellation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// The backend refused the complete explicit row without durable writes.
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
    let target = local_insert_table(query, editor.requires_field_list(), budget, cancelled)?;

    let policy = editor.schema_policy();
    let columns = editor.columns();
    if editor.name().len() > SOURCE_SCHEMA_POLICY.max_name_bytes {
        return Err(QueryError::StructureLimit.into());
    }
    if !equal_name(policy.names, target, editor.name(), budget, cancelled)? {
        return Err(QueryError::UnknownName.into());
    }

    if columns.is_empty()
        || columns.len() > SOURCE_SCHEMA_POLICY.max_columns
        || columns.len() > policy.max_columns
    {
        return Err(QueryError::Width.into());
    }

    // Bound and validate every identity before any quadratic comparison work.
    // Column constructors already establish the shared nonempty/control-free law.
    for column in columns {
        check(budget, cancelled)?;
        policy
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
        SOURCE_SCHEMA_POLICY
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
    }

    for (position, column) in columns.iter().enumerate() {
        for prior in &columns[..position] {
            if equal_name(policy.names, prior.name(), column.name(), budget, cancelled)? {
                return Err(QueryError::AmbiguousName.into());
            }
        }
    }

    let insert = shape::values(query)?;
    let InsertSource::Values(expressions) = &insert.source else {
        return Err(QueryError::Unsupported.into());
    };
    if expressions.len() != columns.len() {
        return Err(QueryError::Width.into());
    }

    budget
        .charge_allocation(columns.len() as u64, size_of::<usize>() as u64)
        .map_err(QueryError::from)?;
    let mut mapping = Vec::new();
    mapping
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    for position in 0..expressions.len() {
        check(budget, cancelled)?;
        let target = if insert.fields.is_empty() {
            position
        } else {
            let mut found = None;
            for (candidate, column) in columns.iter().enumerate() {
                if equal_name(
                    policy.names,
                    column.name(),
                    insert.fields[position].as_str(),
                    budget,
                    cancelled,
                )? {
                    found = Some(candidate);
                    break;
                }
            }
            found.ok_or(QueryError::UnknownName)?
        };
        budget
            .charge(Resource::Steps, mapping.len() as u64)
            .map_err(QueryError::from)?;
        if mapping.contains(&target) {
            return Err(QueryError::DuplicateInsertField.into());
        }
        mapping.push(target);
    }

    budget
        .charge_allocation(columns.len() as u64, size_of::<Option<Value>>() as u64)
        .map_err(QueryError::from)?;
    let mut staged = Vec::new();
    staged
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    staged.resize_with(columns.len(), || None);

    for (expression, target) in expressions.iter().zip(mapping) {
        check(budget, cancelled)?;
        let value = Evaluator::new(&NoBindings, budget, EvalLimits::default())
            .with_cancellation(cancelled)
            .evaluate(expression)
            .map_err(QueryError::from)?;
        staged[target] = Some(stored_value(value, &columns[target])?);
    }

    budget
        .charge_allocation(columns.len() as u64, size_of::<Value>() as u64)
        .map_err(QueryError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    for value in staged {
        check(budget, cancelled)?;
        values.push(value.ok_or(QueryError::Width)?);
    }

    budget.charge(Resource::Rows, 1).map_err(QueryError::from)?;
    check(budget, cancelled)?;

    editor
        .plan_insert(values, budget, cancelled)
        .map_err(InsertError::Backend)
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }

    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn equal_name(
    policy: NameComparison,
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    check(budget, cancelled)?;

    // Equality first checks width, then at most this many UTF-8 bytes. Pay before
    // the comparison, including for duplicate schema and reordered field scans.
    budget.charge(Resource::Steps, left.len().min(right.len()) as u64)?;
    Ok(policy.equals(left, right))
}
