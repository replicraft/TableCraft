use super::{Evaluator, literals::number};
use crate::{
    EvalError,
    comparison::Comparison,
    logic::Truth,
    numeric,
    predicates::{self, Membership},
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_sql::select::Expression as E;

impl Evaluator<'_> {
    pub(super) fn check(&mut self, depth: usize) -> Result<(), EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        if depth > self.limits.max_depth {
            return Err(EvalError::DepthLimit);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }

    pub(super) fn binding_value(&mut self, value: &Value) -> Result<Value, EvalError> {
        match value {
            Value::Text(text) => self.text(text),
            Value::Binary(_) => Err(EvalError::UnsupportedExpression),
            Value::Single(_) | Value::Double(_) => {
                numeric::float(value)?;
                Ok(value.clone())
            }
            _ => Ok(value.clone()),
        }
    }

    pub(super) fn node(&mut self, expression: &E, depth: usize) -> Result<Value, EvalError> {
        self.check(depth)?;

        match expression {
            E::Null => Ok(Value::Null),
            E::Boolean(value) => Ok(Value::Boolean(*value)),
            E::Integer(value) => Ok(Value::Int64(*value)),
            E::Text(text) => self.text(text),
            E::Number(text) => number(text),
            E::GuidLiteral(text) => crate::guid::parse_literal(text).map(Value::Guid),
            E::DateLiteral(text) => crate::datetime::parse_literal(text).map(Value::DateTime),
            E::Field(field) => {
                let value = self.bindings.resolve(field)?;
                self.binding_value(value)
            }
            E::Unary {
                operator,
                expression,
            } => {
                let value = self.node(expression, depth + 1)?;
                self.apply_unary(*operator, value)
            }
            E::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.node(left, depth + 1)?;
                let right = self.node(right, depth + 1)?;
                self.apply_binary(*operator, &left, &right)
            }
            E::IsNull {
                expression,
                negated,
            } => {
                let value = self.expression_operand(expression, depth + 1)?;
                Ok(Value::Boolean(
                    matches!(
                        value,
                        tablecraft_core::values::ExpressionValue::Value(Value::Null)
                    ) != *negated,
                ))
            }
            E::Between {
                expression,
                first_bound,
                second_bound,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                let first = self.node(first_bound, depth + 1)?;
                let second = self.node(second_bound, depth + 1)?;
                predicates::between(&value, &first, &second, *negated, |op, a, b| {
                    self.compare_values(op, a, b)
                })
                .map(Truth::into_value)
            }
            E::InList {
                expression,
                values,
                negated,
            } => {
                if values.is_empty() {
                    return Err(EvalError::UnsupportedExpression);
                }
                let value = self.node(expression, depth + 1)?;
                let mut membership = Membership::new();
                for item in values {
                    let candidate = self.node(item, depth + 1)?;
                    membership.consider(&value, &candidate, |a, b| {
                        self.compare_values(Comparison::Equal, a, b)
                    })?;
                }
                Ok(membership.finish(*negated).into_value())
            }
            E::Like {
                expression,
                pattern,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                let pattern = self.node(pattern, depth + 1)?;
                self.apply_like(&value, &pattern, *negated)
            }
            E::Call {
                qualifier,
                name,
                arguments,
            } => self.call(qualifier, name, arguments, depth),
            E::Identity
            | E::Member { .. }
            | E::Subquery(_)
            | E::Exists(_)
            | E::InQuery { .. }
            | E::Quantified { .. } => Err(EvalError::UnsupportedExpression),
        }
    }
}
