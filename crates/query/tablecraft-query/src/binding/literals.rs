//! Reuse scalar literal parsing without opening the general expression evaluator.
use super::Binder;
use crate::{QueryError, plan::Expr};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_expressions::EvalError;
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_sql::select::Expression;

const LITERAL_EVALUATION_STEPS: u64 = 1;
const MAX_INTEGER_LITERAL_DIGITS: usize = 128;

impl Binder<'_> {
    pub(super) fn wide_integer_literal(&mut self, text: &str) -> Result<Expr, QueryError> {
        if text.len() > MAX_INTEGER_LITERAL_DIGITS {
            return Err(EvalError::UnsupportedLiteral.into());
        }
        if let Ok(value) = text.parse::<i64>() {
            return self.integer_literal(value);
        }

        // Number tokens larger than Long select Double, not an exact LongLong.
        // Capture the nonallocating carrier directly: an internal cast wrapper
        // would consume additional structural depth for a single source token.
        let number = text
            .parse::<f64>()
            .map_err(|_| EvalError::UnsupportedLiteral)?;
        if !number.is_finite() {
            return Err(EvalError::Overflow.into());
        }

        Ok(Expr::NumericLiteral(Value::Double(Double::from_float(
            number,
        ))))
    }

    pub(super) fn integer_literal(&mut self, value: i64) -> Result<Expr, QueryError> {
        // The syntax adapter folds signs. Select from unsigned magnitude as the
        // Number Tokens table does, then retain the already folded signed value.
        let magnitude = value.unsigned_abs();
        let value = if magnitude <= i16::MAX as u64 {
            Value::Int16(i16::try_from(value).map_err(|_| QueryError::Source)?)
        } else if magnitude <= i32::MAX as u64 {
            Value::Int32(i32::try_from(value).map_err(|_| QueryError::Source)?)
        } else {
            Value::Double(Double::from_float(value as f64))
        };

        Ok(Expr::NumericLiteral(value))
    }
}

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
