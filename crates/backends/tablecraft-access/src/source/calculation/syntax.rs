use super::CalculationError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_schema::Column;
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, SourceLocation, parse_expression_strict,
    query::{Query, ReadQuery},
    select::{
        BinaryOperator, Expression, FunctionArgument, Name, Projection, Select, Table, TableSource,
        UnaryOperator,
    },
};

const IIF_ARGUMENTS: usize = 3;
const QUERY_PREFIX: &str = "SELECT 0 AS Recomputed FROM [";
const QUERY_MODEL_BYTES: usize = size_of::<Query>()
    + size_of::<ReadQuery>()
    + size_of::<Select>()
    + size_of::<Projection>()
    + size_of::<TableSource>();

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn prepare(
    table: &str,
    expression: &str,
    columns: &[Column],
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Query, Vec<String>), CalculationError> {
    let expression = parse(expression, limits, budget, cancelled)?;
    prepare_expression(table, expression, columns, budget, cancelled)
}

pub(super) fn parse(
    expression: &str,
    limits: ParseLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Expression, CalculationError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, expression.len() as u64)
        .map_err(QueryError::from)?;
    let expression = parse_expression_strict(expression, limits)?;
    // Parser workspace has independent checked byte/token/depth ceilings.
    // Retained owned syntax and dependency storage additionally consume the
    // caller's native pool; parser scratch memory uses its structural ceilings.
    check(budget, cancelled)?;
    Ok(expression)
}

pub(super) fn prepare_expression(
    table: &str,
    expression: Expression,
    columns: &[Column],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Query, Vec<String>), CalculationError> {
    let mut dependencies = Vec::new();
    validate(&expression, columns, &mut dependencies, budget, cancelled)?;

    // Preserve the existing table-selector policy and conservative model charge.
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

    if length > ParseLimits::MAX_BYTES {
        return Err(ParseError {
            kind: ParseErrorKind::Limit,
            message: "SQL byte limit exceeded",
            location: SourceLocation { line: 1, column: 1 },
        }
        .into());
    }

    let table = Name::from_identifier(table, ParseLimits::default())?;
    let alias = Name::from_identifier("Recomputed", ParseLimits::default())?;
    let query = Query::from_select(Select::from_projection(
        vec![Projection::Expression {
            expression,
            alias: Some(alias),
        }],
        vec![TableSource::Table(Table {
            database: None,
            name: table,
            alias: None,
        })],
    ));

    check(budget, cancelled)?;
    Ok((query, dependencies))
}

pub(super) fn admitted_iif(
    qualifier: &[Name],
    name: &Name,
    arguments: &[FunctionArgument],
) -> bool {
    qualifier.is_empty()
        && name.as_str().eq_ignore_ascii_case("IIf")
        && arguments.len() == IIF_ARGUMENTS
        && arguments
            .iter()
            .all(|argument| matches!(argument, FunctionArgument::Expression(_)))
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
        Expression::Call {
            qualifier,
            name,
            arguments,
        } if admitted_iif(qualifier, name, arguments) => {
            budget
                .charge_allocation(arguments.len() as u64, size_of::<FunctionArgument>() as u64)
                .map_err(QueryError::from)?;
            budget
                .charge_allocation(name.as_str().len() as u64, 1)
                .map_err(QueryError::from)?;
            for argument in arguments {
                let FunctionArgument::Expression(expression) = argument else {
                    return Err(CalculationError::Unsupported);
                };
                validate(expression, columns, dependencies, budget, cancelled)?;
            }
        }
        Expression::Unary {
            operator: UnaryOperator::Plus | UnaryOperator::Minus | UnaryOperator::Not,
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
                | BinaryOperator::Concatenate
                | BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Xor
                | BinaryOperator::Equivalence
                | BinaryOperator::Implication,
        } => {
            validate(left, columns, dependencies, budget, cancelled)?;
            validate(right, columns, dependencies, budget, cancelled)?;
        }
        _ => return Err(CalculationError::Unsupported),
    }
    Ok(())
}
