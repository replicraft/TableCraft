//! Paid borrowed syntax inspection precedes named/external source resolution.
use crate::QueryError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{ReadBody, ReadQuery},
    select::{Expression, FunctionArgument, Projection, TableSource},
};

enum Visit<'a> {
    Expression(&'a Expression, bool),
    Read(&'a ReadQuery),
}

pub(crate) fn predicate(
    value: &Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    validate(value, true, true, budget, cancelled)
}

pub(crate) fn assignment(
    value: &Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    validate(value, false, false, budget, cancelled)
}

pub(crate) fn criteria(
    value: &Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    validate(value, false, true, budget, cancelled)
}

fn validate(
    value: &Expression,
    reads: bool,
    like: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    let mut pending = Vec::new();
    push(
        &mut pending,
        Visit::Expression(value, like),
        budget,
        cancelled,
    )?;
    while let Some(value) = pending.pop() {
        tick(budget, cancelled)?;
        match value {
            Visit::Expression(value, like) => match value {
                Expression::Field(_)
                | Expression::Integer(_)
                | Expression::Text(_)
                | Expression::Number(_)
                | Expression::DateLiteral(_)
                | Expression::GuidLiteral(_)
                | Expression::Null
                | Expression::Boolean(_) => {}
                Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
                    push(
                        &mut pending,
                        Visit::Expression(expression, like),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::Binary { left, right, .. } => {
                    push(
                        &mut pending,
                        Visit::Expression(right, like),
                        budget,
                        cancelled,
                    )?;
                    push(
                        &mut pending,
                        Visit::Expression(left, like),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::Like {
                    expression,
                    pattern,
                    ..
                } if like => {
                    push(
                        &mut pending,
                        Visit::Expression(pattern, like),
                        budget,
                        cancelled,
                    )?;
                    push(
                        &mut pending,
                        Visit::Expression(expression, like),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::Between {
                    expression,
                    first_bound,
                    second_bound,
                    ..
                } => {
                    push(
                        &mut pending,
                        Visit::Expression(second_bound, like),
                        budget,
                        cancelled,
                    )?;
                    push(
                        &mut pending,
                        Visit::Expression(first_bound, like),
                        budget,
                        cancelled,
                    )?;
                    push(
                        &mut pending,
                        Visit::Expression(expression, like),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::InList {
                    expression, values, ..
                } => {
                    for value in values.iter().rev() {
                        push(
                            &mut pending,
                            Visit::Expression(value, like),
                            budget,
                            cancelled,
                        )?;
                    }
                    push(
                        &mut pending,
                        Visit::Expression(expression, like),
                        budget,
                        cancelled,
                    )?;
                }
                Expression::Call { arguments, .. } => {
                    for argument in arguments.iter().rev() {
                        tick(budget, cancelled)?;
                        if let FunctionArgument::Expression(value) = argument {
                            push(
                                &mut pending,
                                Visit::Expression(value, like),
                                budget,
                                cancelled,
                            )?;
                        }
                    }
                }
                Expression::Exists(query) if reads => {
                    push(&mut pending, Visit::Read(query), budget, cancelled)?
                }
                Expression::InQuery {
                    expression, query, ..
                } if reads => {
                    push(&mut pending, Visit::Read(query), budget, cancelled)?;
                    push(
                        &mut pending,
                        Visit::Expression(expression, like),
                        budget,
                        cancelled,
                    )?;
                }
                _ => return Err(QueryError::Unsupported),
            },
            Visit::Read(query) => {
                let ReadBody::Select(select) = &query.body else {
                    return Err(QueryError::Unsupported);
                };
                if query.owner_access
                    || !query.order_by.is_empty()
                    || select.external.is_some()
                    || select.into.is_some()
                    || select.into_external.is_some()
                    || select.owner_access
                    || select.predicate != tablecraft_sql::select::SelectPredicate::All
                    || !select.group_by.is_empty()
                    || select.having.is_some()
                    || !select.order_by.is_empty()
                {
                    return Err(QueryError::Unsupported);
                }
                let [TableSource::Table(table)] = select.from.as_slice() else {
                    return Err(QueryError::Unsupported);
                };
                if table.database.is_some() {
                    return Err(QueryError::Unsupported);
                }
                for projection in select.projection.iter().rev() {
                    tick(budget, cancelled)?;
                    if let Projection::Expression { expression, .. } = projection {
                        push(
                            &mut pending,
                            Visit::Expression(expression, false),
                            budget,
                            cancelled,
                        )?;
                    }
                }
                if let Some(value) = &select.selection {
                    push(
                        &mut pending,
                        Visit::Expression(value, true),
                        budget,
                        cancelled,
                    )?;
                }
                // Bound-plan admission and the normal Binder retain actual
                // structural/read-scope limits. This preflight adds no new one.
            }
        }
    }
    Ok(())
}

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
fn push<'a>(
    pending: &mut Vec<Visit<'a>>,
    value: Visit<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    tick(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let added = pending.capacity().max(1);
        budget.charge(Resource::Steps, pending.len() as u64)?;
        budget.charge_allocation(added as u64, size_of::<Visit<'_>>() as u64)?;
        pending
            .try_reserve_exact(added)
            .map_err(|_| QueryError::Allocation)?;
    }
    pending.push(value);
    Ok(())
}
