//! Criterion shorthand receives a captured operand AST, never an output alias.
use super::super::QueryDesignError;
use tablecraft_sql::{
    ParseLimits, parse_field_rule,
    select::{Expression, FunctionArgument},
};

pub(super) fn parse(text: &str, current: &Expression) -> Result<Expression, QueryDesignError> {
    let mut index = 0_u32;
    let contextual = loop {
        let candidate = format!("__tablecraft_grid_operand_{index}");
        if !text
            .as_bytes()
            .windows(candidate.len())
            .any(|part| part.eq_ignore_ascii_case(candidate.as_bytes()))
        {
            break candidate;
        }
        index = index.checked_add(1).ok_or(QueryDesignError::Limit)?;
    };
    let mut parsed = parse_field_rule(text, &contextual, ParseLimits::default())?;
    substitute(&mut parsed, &contextual, current);
    Ok(parsed)
}

fn substitute(value: &mut Expression, contextual: &str, current: &Expression) {
    match value {
        Expression::Field(field)
            if field.qualifier.is_none()
                && field.name.as_str().eq_ignore_ascii_case(contextual) =>
        {
            // NO_CLONE: Every contextual comparison owns its captured operand
            // AST; the borrowed grid operand is needed for other criteria.
            *value = current.clone();
        }
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            substitute(expression, contextual, current);
        }
        Expression::Binary { left, right, .. } => {
            substitute(left, contextual, current);
            substitute(right, contextual, current);
        }
        Expression::Like {
            expression,
            pattern,
            ..
        } => {
            substitute(expression, contextual, current);
            substitute(pattern, contextual, current);
        }
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            substitute(expression, contextual, current);
            substitute(first_bound, contextual, current);
            substitute(second_bound, contextual, current);
        }
        Expression::InList {
            expression, values, ..
        } => {
            substitute(expression, contextual, current);
            for value in values {
                substitute(value, contextual, current);
            }
        }
        Expression::Call { arguments, .. } => {
            for argument in arguments {
                if let FunctionArgument::Expression(value) = argument {
                    substitute(value, contextual, current);
                }
            }
        }
        _ => {}
    }
}
