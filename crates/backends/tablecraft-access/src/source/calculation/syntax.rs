use super::CalculationError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_schema::Column;
use tablecraft_sql::{
    ParseLimits, parse_expression_strict, parse_query,
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{BinaryOperator, Expression, Projection, Select, TableSource, UnaryOperator},
};

const QUERY_PREFIX: &str = "SELECT 0 AS Recomputed FROM [";
const QUERY_MODEL_BYTES: usize = size_of::<Query>()
    + size_of::<ReadQuery>()
    + size_of::<Select>()
    + size_of::<Projection>()
    + size_of::<TableSource>();

#[cfg(test)]
mod tests;

pub(super) fn prepare(
    table: &str,
    expression: &str,
    columns: &[Column],
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Query, Vec<String>), CalculationError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, expression.len() as u64)
        .map_err(QueryError::from)?;
    let expression = parse_expression_strict(expression, limits)?;
    // Parser workspace has independent checked byte/token/depth ceilings.
    // Retained owned syntax and dependency storage additionally consume the
    // caller's native pool below; this is not a bound on parser scratch memory.
    let mut dependencies = Vec::new();
    validate(&expression, columns, &mut dependencies, budget, cancelled)?;

    // The parser currently has no public arbitrary Name constructor. Admit only
    // a single checked bracket token; never interpolate a formula into SQL text.
    if table.contains(']') || table.chars().any(char::is_control) {
        return Err(CalculationError::Unsupported);
    }
    let length = QUERY_PREFIX
        .len()
        .checked_add(table.len())
        .and_then(|length| length.checked_add("]".len()))
        .ok_or(QueryError::StructureLimit)?;
    budget
        .charge_allocation(length as u64, 1)
        .map_err(QueryError::from)?;
    budget
        .charge_allocation(1, QUERY_MODEL_BYTES as u64)
        .map_err(QueryError::from)?;
    budget
        .charge_allocation((table.len() + "Recomputed".len()) as u64, 1)
        .map_err(QueryError::from)?;
    let mut skeleton = String::new();
    skeleton
        .try_reserve_exact(length)
        .map_err(|_| QueryError::Allocation)?;
    skeleton.push_str(QUERY_PREFIX);
    skeleton.push_str(table);
    skeleton.push(']');
    let mut query = parse_query(&skeleton, ParseLimits::default())?.into_query();
    let Statement::Read(read) = &mut query.statement else {
        return Err(CalculationError::Unsupported);
    };
    let ReadBody::Select(select) = &mut read.body else {
        return Err(CalculationError::Unsupported);
    };
    let Some(Projection::Expression {
        expression: slot, ..
    }) = select.projection.first_mut()
    else {
        return Err(CalculationError::Unsupported);
    };
    *slot = expression;
    check(budget, cancelled)?;
    Ok((query, dependencies))
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CalculationError> {
    if cancelled() {
        return Err(QueryError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    Ok(())
}

fn validate(
    expression: &Expression,
    columns: &[Column],
    dependencies: &mut Vec<String>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CalculationError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(1, size_of::<Expression>() as u64)
        .map_err(QueryError::from)?;
    match expression {
        Expression::Integer(_) | Expression::Null | Expression::Boolean(_) => {}
        Expression::Number(value) | Expression::Text(value) => {
            budget
                .charge_allocation(value.len() as u64, 1)
                .map_err(QueryError::from)?;
        }
        Expression::Field(field) if field.qualifier.is_none() => {
            budget
                .charge(Resource::Steps, (columns.len() + dependencies.len()) as u64)
                .map_err(QueryError::from)?;
            let column = columns
                .iter()
                .find(|column| column.name().eq_ignore_ascii_case(field.name.as_str()))
                .ok_or(CalculationError::Unsupported)?;
            if !dependencies
                .iter()
                .any(|name| name.eq_ignore_ascii_case(column.name()))
            {
                budget
                    .charge_allocation(1, size_of::<String>() as u64)
                    .map_err(QueryError::from)?;
                budget
                    .charge_allocation(column.name().len() as u64, 1)
                    .map_err(QueryError::from)?;
                dependencies
                    .try_reserve_exact(1)
                    .map_err(|_| QueryError::Allocation)?;
                let mut name = String::new();
                name.try_reserve_exact(column.name().len())
                    .map_err(|_| QueryError::Allocation)?;
                name.push_str(column.name());
                dependencies.push(name);
            }
            budget
                .charge_allocation(field.name.as_str().len() as u64, 1)
                .map_err(QueryError::from)?;
        }
        Expression::Unary {
            operator: UnaryOperator::Plus | UnaryOperator::Minus,
            expression,
        }
        | Expression::IsNull { expression, .. } => {
            validate(expression, columns, dependencies, budget, cancelled)?;
        }
        Expression::Binary {
            left,
            right,
            operator:
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::IntegerDivide
                | BinaryOperator::Modulo
                | BinaryOperator::Power
                | BinaryOperator::Concatenate,
        } => {
            validate(left, columns, dependencies, budget, cancelled)?;
            validate(right, columns, dependencies, budget, cancelled)?;
        }
        _ => return Err(CalculationError::Unsupported),
    }
    Ok(())
}
