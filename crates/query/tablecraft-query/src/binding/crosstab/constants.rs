//! Policy-independent fixed headings; reuse scalar kernels without source access.
mod arithmetic;
mod text;

use super::super::Binder;
use crate::{
    QueryError,
    plan::Expr,
    profile::{MAX_BIND_BYTES, MAX_BIND_NODES, MAX_TEXT_BYTES},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
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
            Expr::NumericLiteral(value) => Ok(value.clone()),
            Expr::DateLiteral(_, value) => Ok(Value::DateTime(*value)),
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
                self.constant_unary(*operator, input, value, depth + 1)
            }
            Expr::Binary(BinaryOperator::Add | BinaryOperator::Concatenate, ..)
            | Expr::TextCall(..)
            | Expr::CVar(_)
            | Expr::IIf(..)
            | Expr::Nz(..)
            | Expr::ConditionalCall(..) => self.constant_text_heading(expr, depth + 1),
            Expr::Binary(operator, left, right) => {
                let expressions = (left.as_ref(), right.as_ref());
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
                self.constant_binary(*operator, expressions, (&left, &right), bytes, depth + 1)
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
            // Fields, parameters, subqueries and policy/host-dependent calls cannot
            // declare a stable metadata schema. No evaluator or source sees them.
            _ => Err(QueryError::Unsupported),
        }
    }

    fn heading_operation<T>(
        &mut self,
        bytes: usize,
        operation: impl FnOnce(&mut Evaluator<'_>) -> Result<T, EvalError>,
    ) -> Result<T, QueryError> {
        // Reserve the bounded allocation allowance in the owning binding budget
        // before allocation. Every kernel additionally enforces this allowance.
        // Every recursive node already consumes the global binding work/depth quota.
        self.tick(1)?;
        self.charge(bytes)?;
        let steps = self
            .budget
            .as_ref()
            .map_or(MAX_BIND_NODES as u64, |budget| {
                budget.remaining(Resource::Steps).min(MAX_BIND_NODES as u64)
            });
        let mut allowance = ResourceBudget::new(bytes as u64, 0, steps);
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
        let result = operation(&mut evaluator);
        let used_steps = steps - allowance.remaining(Resource::Steps);
        if let Some(budget) = &mut self.budget {
            // The exclusive local allowance cannot exceed the shared remainder.
            // Debit work on failures too; no kernel receives a refreshed pool.
            budget.charge(Resource::Steps, used_steps)?;
        }
        result.map_err(|error| match error {
            EvalError::Cancelled => QueryError::Cancelled,
            error => QueryError::Evaluation(error),
        })
    }
}

#[cfg(test)]
mod tests;
