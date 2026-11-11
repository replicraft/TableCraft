//! Paid syntax-only read-mode discovery for local UPDATE and DELETE.
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_sql::select::{Expression, FunctionArgument};

pub(super) fn uses_reads<'a>(
    expressions: impl IntoIterator<Item = &'a Expression>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    let mut pending = Vec::new();
    for value in expressions {
        push(&mut pending, value, budget, cancelled)?;
    }
    let mut selected = false;
    while let Some(value) = pending.pop() {
        check(budget, cancelled)?;
        match value {
            Expression::Exists(_) | Expression::Subquery(_) => selected = true,
            Expression::InQuery { expression, .. } | Expression::Quantified { expression, .. } => {
                selected = true;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Unary { expression, .. }
            | Expression::IsNull { expression, .. }
            | Expression::Member { expression, .. } => {
                push(&mut pending, expression, budget, cancelled)?
            }
            Expression::Binary { left, right, .. } => {
                push(&mut pending, right, budget, cancelled)?;
                push(&mut pending, left, budget, cancelled)?;
            }
            Expression::Like {
                expression,
                pattern,
                ..
            } => {
                push(&mut pending, pattern, budget, cancelled)?;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                ..
            } => {
                push(&mut pending, second_bound, budget, cancelled)?;
                push(&mut pending, first_bound, budget, cancelled)?;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::InList {
                expression, values, ..
            } => {
                for value in values.iter().rev() {
                    push(&mut pending, value, budget, cancelled)?;
                }
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Call { arguments, .. } => {
                for argument in arguments.iter().rev() {
                    check(budget, cancelled)?;
                    if let FunctionArgument::Expression(value) = argument {
                        push(&mut pending, value, budget, cancelled)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(selected)
}
pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<&'a Expression>,
    value: &'a Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    check(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let added = pending.capacity().max(1);
        budget.charge(Resource::Steps, pending.len() as u64)?;
        budget.charge_allocation(added as u64, size_of::<&Expression>() as u64)?;
        pending
            .try_reserve_exact(added)
            .map_err(|_| QueryError::Allocation)?;
    }
    pending.push(value);
    Ok(())
}
