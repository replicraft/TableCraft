//! Reuse scalar literal parsing without opening the general expression evaluator.
use crate::QueryError;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_sql::select::Expression;

const LITERAL_EVALUATION_STEPS: u64 = 1;

/// Only closed, nonallocating literal forms may enter this helper. It has no
/// field bindings, clock, random source, callable provider or host context.
pub(super) fn checked(expression: &Expression) -> Result<Value, QueryError> {
    if !matches!(
        expression,
        Expression::Number(_) | Expression::DateLiteral(_) | Expression::GuidLiteral(_)
    ) {
        return Err(QueryError::Unsupported);
    }
    let mut budget = ResourceBudget::new(0, 0, LITERAL_EVALUATION_STEPS);
    Ok(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression)?)
}
