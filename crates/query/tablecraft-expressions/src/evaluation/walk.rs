use super::{Evaluator, arithmetic_unary_operand, literals::number};
use crate::{
    EvalError,
    comparison::Comparison,
    functions::conversions::errors::into_operator_scalar,
    logic::Truth,
    numeric,
    predicates::{self, Membership},
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_sql::select::{BinaryOperator, Expression as E, UnaryOperator};

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
            E::DateLiteral(text) => {
                let value = match self.reference_year {
                    Some(year) => {
                        crate::datetime::parse_query_literal_with_reference_year(text, year)
                    }
                    None => crate::datetime::parse_query_literal(text),
                }?;
                Ok(Value::DateTime(value))
            }
            E::Field(field) => {
                let value = self.bindings.resolve(field)?;
                self.binding_value(value)
            }
            E::Unary {
                operator,
                expression,
            } => {
                let value = self.expression_operand(expression, depth + 1)?;
                let value = if matches!(operator, UnaryOperator::Plus | UnaryOperator::Minus) {
                    arithmetic_unary_operand(*operator, value)?
                } else {
                    into_operator_scalar(value)?
                };
                self.apply_unary(*operator, value)
            }
            E::Binary {
                left,
                operator,
                right,
            } => {
                if matches!(
                    operator,
                    BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::IntegerDivide
                        | BinaryOperator::Modulo
                        | BinaryOperator::Power
                ) {
                    return self.numeric_arithmetic_call(*operator, left, right, depth);
                }
                if *operator == BinaryOperator::Add {
                    let value = self.addition_call(left, right, depth)?;
                    return self.publish_byte_operand(value);
                }
                if *operator == BinaryOperator::Concatenate {
                    let value = self.concatenation_call(left, right, depth)?;
                    return self.publish_byte_operand(value);
                }
                let left = self.expression_operand(left, depth + 1)?;
                let right = self.expression_operand(right, depth + 1)?;
                self.apply_expression_binary(*operator, left, right)
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
                let value = self.expression_operand(expression, depth + 1)?;
                let first = self.expression_operand(first_bound, depth + 1)?;
                let second = self.expression_operand(second_bound, depth + 1)?;
                predicates::between(&value, &first, &second, *negated, |op, a, b| {
                    self.compare_expression_values(op, a, b)
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
                let value = self.expression_operand(expression, depth + 1)?;
                let mut membership = Membership::new();
                for item in values {
                    let candidate = self.expression_operand(item, depth + 1)?;
                    membership.consider(&value, &candidate, |a, b| {
                        self.compare_expression_values(Comparison::Equal, a, b)
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
