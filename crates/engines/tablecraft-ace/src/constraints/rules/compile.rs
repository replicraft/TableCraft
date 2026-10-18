use crate::{InspectError, TableSnapshot, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression, parse_field_rule,
    select::{BinaryOperator, Expression},
};

const MAX_RULE_BYTES: usize = 4096;
const MAX_RULE_TOKENS: usize = 128;
const MAX_RULE_DEPTH: usize = 16;

pub(super) fn rule(
    table: &TableSnapshot,
    text: &str,
    field: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Expression>, InspectError> {
    check(budget, cancelled)?;
    if text.len() > MAX_RULE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    let field_name = field
        .map(|position| {
            table
                .columns()
                .get(position)
                .map(|column| column.name())
                .ok_or(InspectError::Malformed)
        })
        .transpose()?;
    // Reserve an AST/node/name allowance before parsing. Upstream tokenization's
    // temporary buffers have separate byte/token/depth ceilings, not an allocator
    // quota supplied by ResourceBudget.
    budget.charge_allocation(MAX_RULE_TOKENS as u64, size_of::<Expression>() as u64)?;
    budget.charge_allocation(
        MAX_RULE_TOKENS as u64,
        field_name.map_or(0, str::len) as u64,
    )?;
    budget.charge_allocation(text.len() as u64, 2)?;
    let limits = ParseLimits {
        max_bytes: MAX_RULE_BYTES,
        max_tokens: MAX_RULE_TOKENS,
        max_depth: MAX_RULE_DEPTH,
    };
    let parsed = match field_name {
        Some(name) => parse_field_rule(text, name, limits),
        None => parse_expression(text, limits),
    };
    let expression = match parsed {
        Ok(expression) => expression,
        Err(error) if error.kind == ParseErrorKind::Limit => {
            return Err(InspectError::StructureLimit);
        }
        Err(_) => return Ok(None),
    };
    check(budget, cancelled)?;
    if !predicate(&expression) || !supported(&expression, table, field, budget, cancelled)? {
        return Ok(None);
    }
    Ok(Some(expression))
}

fn predicate(expression: &Expression) -> bool {
    match expression {
        Expression::Boolean(_) | Expression::IsNull { .. } => true,
        Expression::Unary {
            operator: tablecraft_sql::select::UnaryOperator::Not,
            ..
        } => true,
        Expression::Binary { operator, .. } => matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Xor
                | BinaryOperator::Equivalence
                | BinaryOperator::Implication
        ),
        _ => false,
    }
}

fn supported(
    expression: &Expression,
    table: &TableSnapshot,
    field: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    check(budget, cancelled)?;
    Ok(match expression {
        Expression::Null
        | Expression::Boolean(_)
        | Expression::Integer(_)
        | Expression::Number(_)
        | Expression::Text(_)
        | Expression::DateLiteral(_)
        | Expression::GuidLiteral(_) => true,
        Expression::Field(reference) => {
            let position = table
                .columns()
                .iter()
                .position(|column| column.name().eq_ignore_ascii_case(reference.name.as_str()));
            reference
                .qualifier
                .as_ref()
                .is_none_or(|name| name.as_str().eq_ignore_ascii_case(table.schema().name()))
                && position.is_some_and(|position| {
                    field.is_none_or(|selected| selected == position)
                        && table.columns()[position].data_type() != ScalarType::Binary
                })
        }
        Expression::Unary {
            operator,
            expression,
        } => {
            (!matches!(operator, tablecraft_sql::select::UnaryOperator::Not)
                || predicate(expression))
                && supported(expression, table, field, budget, cancelled)?
        }
        Expression::IsNull { expression, .. } => {
            supported(expression, table, field, budget, cancelled)?
        }
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let comparison = matches!(
                operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            );
            let logical = matches!(
                operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Xor
                    | BinaryOperator::Equivalence
                    | BinaryOperator::Implication
            );
            if logical && (!predicate(left) || !predicate(right))
                || comparison && (contains_text(left, table) || contains_text(right, table))
            {
                false
            } else {
                supported(left, table, field, budget, cancelled)?
                    && supported(right, table, field, budget, cancelled)?
            }
        }
        // Calls, subqueries, host members, LIKE and other unimplemented scalar
        // forms are rejected before any record evaluation, including empty tables.
        _ => false,
    })
}

fn contains_text(expression: &Expression, table: &TableSnapshot) -> bool {
    match expression {
        Expression::Text(_) => true,
        Expression::Field(field) => table.columns().iter().any(|column| {
            column.name().eq_ignore_ascii_case(field.name.as_str())
                && column.data_type() == ScalarType::Text
        }),
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            contains_text(expression, table)
        }
        Expression::Binary { left, right, .. } => {
            contains_text(left, table) || contains_text(right, table)
        }
        _ => false,
    }
}
