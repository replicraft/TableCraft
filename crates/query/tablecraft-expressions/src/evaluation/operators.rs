use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison as C},
    logic::Truth,
    numeric::{self, Arithmetic as A},
    text,
};
use tablecraft_core::values::Value;
use tablecraft_sql::select::BinaryOperator as B;

impl Evaluator<'_> {
    pub(super) fn binary(
        &mut self,
        operator: B,
        left: &Value,
        right: &Value,
    ) -> Result<Value, EvalError> {
        let comparison = match operator {
            B::Equal => Some(C::Equal),
            B::NotEqual => Some(C::NotEqual),
            B::Less => Some(C::Less),
            B::LessOrEqual => Some(C::LessOrEqual),
            B::Greater => Some(C::Greater),
            B::GreaterOrEqual => Some(C::GreaterOrEqual),
            _ => None,
        };
        if let Some(operator) = comparison {
            return comparison::compare(operator, left, right, self.collation)
                .map(Truth::into_value);
        }
        if matches!(
            operator,
            B::And | B::Or | B::Xor | B::Equivalence | B::Implication
        ) {
            let (left, right) = (Truth::from_value(left)?, Truth::from_value(right)?);
            return Ok(match operator {
                B::And => left.and(right),
                B::Or => left.or(right),
                B::Xor => left.xor(right),
                B::Equivalence => left.equivalent(right),
                B::Implication => left.implies(right),
                _ => unreachable!(),
            }
            .into_value());
        }
        let text_add = operator == B::Add
            && (matches!(left, Value::Text(_)) || matches!(right, Value::Text(_)));
        if operator == B::Concatenate || text_add {
            let is_null = if text_add {
                matches!(left, Value::Null) || matches!(right, Value::Null)
            } else {
                matches!((left, right), (Value::Null, Value::Null))
            };
            if is_null {
                return Ok(Value::Null);
            }
            let length = |value: &Value| match value {
                Value::Text(value) => Ok(value.len()),
                Value::Null => Ok(0),
                _ => Err(EvalError::TypeMismatch),
            };
            let length = length(left)?
                .checked_add(length(right)?)
                .ok_or(EvalError::TextLimit)?;
            self.charge_text(length)?;
            return if text_add {
                text::plus(left, right, self.limits.max_text_bytes)
            } else {
                text::concatenate(left, right, self.limits.max_text_bytes)
            };
        }
        let operation = match operator {
            B::Add => A::Add,
            B::Subtract => A::Subtract,
            B::Multiply => A::Multiply,
            B::Divide => A::Divide,
            B::IntegerDivide => A::IntegerDivide,
            B::Modulo => A::Modulo,
            B::Power => A::Power,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        numeric::arithmetic(operation, left, right)
    }
}
