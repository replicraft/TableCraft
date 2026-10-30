//! Closed single-row VALUES syntax, checked before metadata or scalar evaluation.
use crate::{
    QueryError,
    profile::{MAX_BIND_BYTES, MAX_BIND_NODES, MAX_TEXT_BYTES, SOURCE_SCHEMA_POLICY},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::evaluation::EvalLimits;
use tablecraft_sql::{
    query::{Insert, InsertSource, Query, Statement},
    select::{Expression, FunctionArgument},
};

/// Resolve closed local INSERT VALUES syntax without opening or scanning a source.
/// Requires one complete VALUES row; SELECT, parameters, external targets,
/// identity reads, field/object references and user functions remain refused.
pub fn local_insert_table<'a>(
    query: &'a Query,
    require_field_list: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a str, QueryError> {
    let insert = values(query)?;
    if require_field_list && insert.fields.is_empty() {
        return Err(QueryError::Unsupported);
    }
    let InsertSource::Values(expressions) = &insert.source else {
        return Err(QueryError::Unsupported);
    };

    let mut nodes = 0;
    let mut bytes = 0;
    for expression in expressions {
        expression_shape(expression, 1, &mut nodes, &mut bytes, budget, cancelled)?;
    }

    Ok(insert.target.as_str())
}

pub(super) fn values(query: &Query) -> Result<&Insert, QueryError> {
    if !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }

    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let InsertSource::Values(expressions) = &insert.source else {
        return Err(QueryError::Unsupported);
    };
    if insert.external.is_some()
        || expressions.is_empty()
        || expressions.len() > SOURCE_SCHEMA_POLICY.max_columns
        || (!insert.fields.is_empty() && insert.fields.len() != expressions.len())
    {
        return Err(QueryError::Unsupported);
    }

    Ok(insert)
}

fn expression_shape(
    expression: &Expression,
    depth: usize,
    nodes: &mut usize,
    bytes: &mut usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }

    budget.charge(Resource::Steps, 1)?;
    *nodes = nodes.checked_add(1).ok_or(QueryError::StructureLimit)?;
    if *nodes > MAX_BIND_NODES || depth > EvalLimits::MAX_DEPTH {
        return Err(QueryError::StructureLimit);
    }
    if let Expression::Text(text)
    | Expression::Number(text)
    | Expression::DateLiteral(text)
    | Expression::GuidLiteral(text) = expression
    {
        *bytes = bytes
            .checked_add(text.len())
            .ok_or(QueryError::StructureLimit)?;
        if text.len() > MAX_TEXT_BYTES || *bytes > MAX_BIND_BYTES {
            return Err(QueryError::StructureLimit);
        }
        // Prepay the raw literal scan before numeric/date/GUID parsing as well
        // as Text evaluation; direct AST callers have no host parser allowance.
        budget.charge(Resource::Steps, text.len() as u64)?;
    }

    let mut child = |expression: &Expression| {
        expression_shape(expression, depth + 1, nodes, bytes, budget, cancelled)
    };
    match expression {
        Expression::Null
        | Expression::Boolean(_)
        | Expression::Integer(_)
        | Expression::Number(_)
        | Expression::Text(_)
        | Expression::DateLiteral(_)
        | Expression::GuidLiteral(_) => Ok(()),
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            child(expression)
        }
        Expression::Binary { left, right, .. } => {
            child(left)?;
            child(right)
        }
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            child(expression)?;
            child(first_bound)?;
            child(second_bound)
        }
        Expression::InList {
            expression, values, ..
        } => {
            child(expression)?;
            for value in values {
                child(value)?;
            }
            Ok(())
        }
        Expression::Call {
            qualifier,
            name,
            arguments,
        } if qualifier.is_empty() && conversion(name.as_str()) => {
            for argument in arguments {
                let FunctionArgument::Expression(expression) = argument else {
                    return Err(QueryError::Unsupported);
                };
                child(expression)?;
            }
            Ok(())
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn conversion(name: &str) -> bool {
    // Existing query-available deterministic carrier constructors only. No
    // ambient clock/locale, dynamic Eval, domains, aggregates or UDF dispatch.
    [
        "CBool", "CByte", "CInt", "CLng", "CCur", "CDec", "CSng", "CDbl",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}
