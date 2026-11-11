//! Prevent named or external resolution before explicit predicate binding.
use crate::{BindingDepth, QueryError};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{ReadBody, ReadQuery},
    select::{Expression, FunctionArgument, Projection, TableSource},
};

pub(super) fn expression(
    value: &Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    depth: usize,
) -> Result<(), QueryError> {
    tick(budget, cancelled, depth)?;
    match value {
        Expression::Field(_)
        | Expression::Integer(_)
        | Expression::Text(_)
        | Expression::Number(_)
        | Expression::DateLiteral(_)
        | Expression::GuidLiteral(_)
        | Expression::Null
        | Expression::Boolean(_) => Ok(()),
        Expression::Unary {
            expression: value, ..
        }
        | Expression::IsNull {
            expression: value, ..
        } => expression(value, budget, cancelled, depth + 1),
        Expression::Binary { left, right, .. } => {
            expression(left, budget, cancelled, depth + 1)?;
            expression(right, budget, cancelled, depth + 1)
        }
        Expression::Between {
            expression: value,
            first_bound,
            second_bound,
            ..
        } => {
            expression(value, budget, cancelled, depth + 1)?;
            expression(first_bound, budget, cancelled, depth + 1)?;
            expression(second_bound, budget, cancelled, depth + 1)
        }
        Expression::InList {
            expression: value,
            values,
            ..
        } => {
            expression(value, budget, cancelled, depth + 1)?;
            for value in values {
                expression(value, budget, cancelled, depth + 1)?;
            }
            Ok(())
        }
        Expression::Call { arguments, .. } => {
            // Actual callable identity/arity remains Binder::mutation's responsibility.
            for argument in arguments {
                tick(budget, cancelled, depth)?;
                if let FunctionArgument::Expression(value) = argument {
                    expression(value, budget, cancelled, depth + 1)?;
                }
            }
            Ok(())
        }
        Expression::Exists(query) => read(query, budget, cancelled, depth + 1),
        Expression::InQuery {
            expression: value,
            query,
            ..
        } => {
            expression(value, budget, cancelled, depth + 1)?;
            read(query, budget, cancelled, depth + 1)
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn read(
    query: &ReadQuery,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    depth: usize,
) -> Result<(), QueryError> {
    tick(budget, cancelled, depth)?;
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
    for projection in &select.projection {
        tick(budget, cancelled, depth)?;
        if let Projection::Expression {
            expression: value, ..
        } = projection
        {
            expression(value, budget, cancelled, depth + 1)?;
        }
    }
    if let Some(value) = &select.selection {
        expression(value, budget, cancelled, depth + 1)?;
    }
    // Remaining relational/aggregate shapes are refused by the paid bound-plan walk.
    Ok(())
}

fn tick(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    depth: usize,
) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    if depth > BindingDepth::DEFAULT {
        return Err(QueryError::StructureLimit);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
