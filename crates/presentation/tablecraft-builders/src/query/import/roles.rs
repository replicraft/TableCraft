//! Import preserves captured operands and explicit aggregate wrappers exactly.
use super::super::build::{expression, name};
use super::super::totals::aggregate;
use super::super::{
    DesignColumn, DesignExpression, DesignTotal, QueryDesignDraft, QueryDesignError,
};
use tablecraft_sql::select::{Expression, FunctionArgument};

pub(super) fn contains_aggregate(value: &Expression) -> bool {
    match value {
        Expression::Call {
            qualifier,
            name,
            arguments,
        } => {
            (qualifier.is_empty()
                && [
                    "COUNT", "SUM", "AVG", "MIN", "MAX", "FIRST", "LAST", "STDEV", "STDEVP", "VAR",
                    "VARP",
                ]
                .iter()
                .any(|aggregate| name.as_str().eq_ignore_ascii_case(aggregate)))
                || arguments.iter().any(|argument| match argument {
                    FunctionArgument::Expression(value) => contains_aggregate(value),
                    _ => false,
                })
        }
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            contains_aggregate(expression)
        }
        Expression::Binary { left, right, .. } => {
            contains_aggregate(left) || contains_aggregate(right)
        }
        Expression::Like {
            expression,
            pattern,
            ..
        } => contains_aggregate(expression) || contains_aggregate(pattern),
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            contains_aggregate(expression)
                || contains_aggregate(first_bound)
                || contains_aggregate(second_bound)
        }
        Expression::InList {
            expression, values, ..
        } => contains_aggregate(expression) || values.iter().any(contains_aggregate),
        _ => false,
    }
}

pub(super) fn imported(
    value: Expression,
    grouped: bool,
    keys: &[Expression],
) -> (DesignExpression, DesignTotal) {
    if !grouped {
        return (
            DesignExpression::Expression(value.to_string()),
            DesignTotal::GroupBy,
        );
    }
    if keys.contains(&value) {
        return (
            DesignExpression::Expression(value.to_string()),
            DesignTotal::GroupBy,
        );
    }
    if let Expression::Call {
        qualifier,
        name,
        arguments,
    } = &value
        && qualifier.is_empty()
    {
        for total in [
            DesignTotal::Sum,
            DesignTotal::Avg,
            DesignTotal::Min,
            DesignTotal::Max,
            DesignTotal::Count,
            DesignTotal::First,
            DesignTotal::Last,
        ] {
            if aggregate(total).is_some_and(|function| name.as_str().eq_ignore_ascii_case(function))
                && let [FunctionArgument::Expression(operand)] = arguments.as_slice()
            {
                return (DesignExpression::Expression(operand.to_string()), total);
            }
        }
    }
    (
        DesignExpression::Expression(value.to_string()),
        DesignTotal::Expression,
    )
}

pub(super) fn operand(
    draft: &QueryDesignDraft,
    column: &DesignColumn,
) -> Result<Expression, QueryDesignError> {
    let value = expression(draft, &column.expression)?;
    Ok(match aggregate(column.total).filter(|_| draft.totals) {
        Some(function) => Expression::Call {
            qualifier: Vec::new(),
            name: name(function)?,
            arguments: vec![FunctionArgument::Expression(value)],
        },
        None => value,
    })
}

fn canonical_aggregate(name: &str) -> Option<&'static str> {
    [
        "Sum", "Avg", "Min", "Max", "Count", "First", "Last", "StDev", "StDevP", "Var", "VarP",
    ]
    .into_iter()
    .find(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn normalize_expression(value: &mut Expression) -> Result<(), QueryDesignError> {
    match value {
        Expression::Call {
            qualifier,
            name: function,
            arguments,
        } => {
            if qualifier.is_empty()
                && let Some(spelling) = canonical_aggregate(function.as_str())
            {
                *function = name(spelling)?;
            }
            for argument in arguments {
                if let FunctionArgument::Expression(value) = argument {
                    normalize_expression(value)?;
                }
            }
        }
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            normalize_expression(expression)?
        }
        Expression::Binary { left, right, .. } => {
            normalize_expression(left)?;
            normalize_expression(right)?;
        }
        Expression::Like {
            expression,
            pattern,
            ..
        } => {
            normalize_expression(expression)?;
            normalize_expression(pattern)?;
        }
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            normalize_expression(expression)?;
            normalize_expression(first_bound)?;
            normalize_expression(second_bound)?;
        }
        Expression::InList {
            expression, values, ..
        } => {
            normalize_expression(expression)?;
            for value in values {
                normalize_expression(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn normalize(
    select: &mut tablecraft_sql::select::Select,
) -> Result<(), QueryDesignError> {
    for projection in &mut select.projection {
        if let tablecraft_sql::select::Projection::Expression { expression, .. } = projection {
            normalize_expression(expression)?;
        }
    }
    for key in &mut select.group_by {
        normalize_expression(key)?;
    }
    if let Some(expression) = &mut select.selection {
        normalize_expression(expression)?;
    }
    if let Some(expression) = &mut select.having {
        normalize_expression(expression)?;
    }
    for key in &mut select.order_by {
        normalize_expression(&mut key.expression)?;
    }
    Ok(())
}
