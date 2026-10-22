//! Policy-independent fixed headings; reuse scalar kernels without source access.
use super::super::Binder;
use crate::{
    QueryError,
    plan::Expr,
    profile::{MAX_BIND_BYTES, MAX_BIND_NODES, MAX_TEXT_BYTES},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::math::MathFunction,
};
use tablecraft_sql::select::{BinaryOperator, Expression};

impl Binder<'_> {
    pub(super) fn constant_heading(
        &mut self,
        expr: &Expr,
        depth: usize,
    ) -> Result<Value, QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Literal(Expression::Text(text)) => Ok(Value::Text(self.copy_name(text)?)),
            Expr::Literal(
                literal @ (Expression::Null
                | Expression::Boolean(_)
                | Expression::Integer(_)
                | Expression::Number(_)
                | Expression::GuidLiteral(_)
                | Expression::DateLiteral(_)),
            ) => self.heading_operation(0, |evaluator| evaluator.evaluate(literal)),
            Expr::Unary(operator, input) => {
                let value = self.constant_heading(input, depth + 1)?;
                if matches!(value, Value::Text(_)) {
                    return Err(QueryError::Unsupported);
                }
                self.heading_operation(0, |evaluator| evaluator.apply_unary(*operator, value))
            }
            Expr::Binary(operator, left, right) => {
                let left = self.constant_heading(left, depth + 1)?;
                let right = self.constant_heading(right, depth + 1)?;
                let has_text = matches!(left, Value::Text(_)) || matches!(right, Value::Text(_));
                let concatenate = *operator == BinaryOperator::Concatenate
                    || (*operator == BinaryOperator::Add
                        && matches!(
                            (&left, &right),
                            (Value::Text(_), Value::Text(_) | Value::Null)
                                | (Value::Null, Value::Text(_))
                        ));
                if has_text && !concatenate {
                    // Comparisons and numeric Text coercion depend on execution policies.
                    return Err(QueryError::Unsupported);
                }
                let bytes = if concatenate {
                    let length = |value: &Value| match value {
                        Value::Text(text) => Ok(text.len()),
                        Value::Null => Ok(0),
                        // Scalar formatting needs execution-local policy, so it
                        // cannot declare metadata headings during binding.
                        _ => Err(QueryError::Unsupported),
                    };
                    length(&left)?
                        .checked_add(length(&right)?)
                        .ok_or(QueryError::StructureLimit)?
                } else {
                    0
                };
                self.heading_operation(bytes, |evaluator| {
                    evaluator.apply_binary(*operator, &left, &right)
                })
            }
            Expr::Convert(conversion, input) => {
                let value = self.constant_heading(input, depth + 1)?;
                if matches!(value, Value::Text(_)) {
                    return Err(QueryError::Unsupported);
                }
                self.heading_operation(0, |evaluator| {
                    evaluator.apply_conversion(*conversion, &value)
                })
            }
            Expr::MathCall(function, arguments) if *function != MathFunction::Rnd => {
                let mut values: [Value; MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                if arguments.len() > values.len() {
                    return Err(QueryError::Unsupported);
                }
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.constant_heading(argument, depth + 1)?;
                    if matches!(value, Value::Text(_)) {
                        return Err(QueryError::Unsupported);
                    }
                }
                self.heading_operation(0, |evaluator| {
                    evaluator.apply_math(*function, &values[..arguments.len()])
                })
            }
            Expr::CVar(input) => self.constant_heading(input, depth + 1),
            // Fields, parameters, subqueries and policy/host-dependent calls cannot
            // declare a stable metadata schema. No evaluator or source sees them.
            _ => Err(QueryError::Unsupported),
        }
    }

    fn heading_operation(
        &mut self,
        bytes: usize,
        operation: impl FnOnce(&mut Evaluator<'_>) -> Result<Value, EvalError>,
    ) -> Result<Value, QueryError> {
        // Reserve the exact possible Text result in the owning binding budget
        // before allocation. Numeric kernels cannot allocate a Text result here.
        // Every recursive node already consumes the global binding work/depth quota.
        self.tick(1)?;
        self.charge(bytes)?;
        let mut allowance = ResourceBudget::new(bytes as u64, 0, MAX_BIND_NODES as u64);
        let mut evaluator = Evaluator::new(
            &NoBindings,
            &mut allowance,
            EvalLimits {
                max_text_bytes: MAX_BIND_BYTES.min(MAX_TEXT_BYTES),
                ..Default::default()
            },
        );
        if let Some(cancelled) = self.cancelled {
            evaluator = evaluator.with_cancellation(cancelled);
        }
        operation(&mut evaluator).map_err(|error| match error {
            EvalError::Cancelled => QueryError::Cancelled,
            error => QueryError::Evaluation(error),
        })
    }
}

#[cfg(test)]
mod tests;
