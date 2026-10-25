use super::{Evaluator, PreparedAddition, PreparedNumericArithmetic};
use crate::{
    EvalError,
    comparison::Comparison as C,
    functions::{
        conversions::{Conversion, errors::into_operator_scalar},
        text::byte_string::ExpressionByteOperand,
    },
    logic::{Logical, Truth},
    numeric::{self, Arithmetic as A},
    text,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

impl Evaluator<'_> {
    /// Apply a binary operator to already evaluated expression data.
    /// Hosts evaluate both operands before this call. Arithmetic admits Empty at
    /// operator-specific targets; ampersand uses empty Text. Logical operators
    /// admit Empty and raw Strings; comparisons admit standard Error/Error pairs.
    /// Arithmetic and concatenation normalize retained active String ranges through
    /// the same preparation and strict publication boundaries as SQL evaluation.
    pub fn apply_expression_binary(
        &mut self,
        operator: B,
        left: ExpressionValue,
        right: ExpressionValue,
    ) -> Result<Value, EvalError> {
        let logical = match operator {
            B::And => Some(Logical::And),
            B::Or => Some(Logical::Or),
            B::Xor => Some(Logical::Xor),
            B::Equivalence => Some(Logical::Equivalence),
            B::Implication => Some(Logical::Implication),
            _ => None,
        };
        if let Some(logical) = logical {
            return self.apply_expression_logical(logical, left, right);
        }

        if operator == B::Add {
            return match self.prepare_expression_addition(
                ExpressionByteOperand::try_from(left)?,
                ExpressionByteOperand::try_from(right)?,
            )? {
                PreparedAddition::StringResult(value) => self.publish_byte_operand(value),
                PreparedAddition::ScalarOperands { left, right } => {
                    self.apply_binary(operator, &left, &right)
                }
            };
        }
        if operator == B::Concatenate {
            let result = self.apply_expression_concatenation(
                ExpressionByteOperand::try_from(left)?,
                ExpressionByteOperand::try_from(right)?,
            )?;
            return self.publish_byte_operand(result);
        }
        let comparison = match operator {
            B::Equal => Some(C::Equal),
            B::NotEqual => Some(C::NotEqual),
            B::Less => Some(C::Less),
            B::LessOrEqual => Some(C::LessOrEqual),
            B::Greater => Some(C::Greater),
            B::GreaterOrEqual => Some(C::GreaterOrEqual),
            _ => None,
        };
        if let Some(comparison) = comparison {
            return self
                .compare_expression_values(comparison, &left, &right)
                .map(Truth::into_value);
        }
        if matches!(
            operator,
            B::Subtract | B::Multiply | B::Divide | B::IntegerDivide | B::Modulo | B::Power
        ) {
            return match self.prepare_expression_numeric_arithmetic(
                operator,
                ExpressionByteOperand::try_from(left)?,
                ExpressionByteOperand::try_from(right)?,
            )? {
                PreparedNumericArithmetic::Result(value) => Ok(value),
                PreparedNumericArithmetic::Operands { left, right } => {
                    self.apply_binary(operator, &left, &right)
                }
            };
        }

        let left = into_operator_scalar(left)?;
        let right = into_operator_scalar(right)?;
        self.apply_binary(operator, &left, &right)
    }

    /// Apply an operator to already evaluated operands. Hosts charge their own
    /// node/row work; Text intermediates consume this evaluator's byte budget.
    pub fn apply_binary(
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
            return self
                .compare_values(operator, left, right)
                .map(Truth::into_value);
        }
        let logical = match operator {
            B::And => Some(Logical::And),
            B::Or => Some(Logical::Or),
            B::Xor => Some(Logical::Xor),
            B::Equivalence => Some(Logical::Equivalence),
            B::Implication => Some(Logical::Implication),
            _ => None,
        };
        if let Some(logical) = logical {
            return self.apply_logical(logical, left, right);
        }
        if operator == B::Concatenate {
            return self.concatenate(left, right);
        }
        let text_add = operator == B::Add
            && matches!(
                (left, right),
                (Value::Text(_), Value::Text(_) | Value::Null) | (Value::Null, Value::Text(_))
            );
        if text_add {
            if matches!(left, Value::Null) || matches!(right, Value::Null) {
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
            return text::plus(left, right, self.limits.max_text_bytes);
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
        self.numeric_binary(operation, left, right)
    }

    /// Apply a prefix operator to an already evaluated scalar operand.
    pub fn apply_unary(&mut self, operator: U, value: Value) -> Result<Value, EvalError> {
        let value = if operator != U::Not && matches!(value, Value::Text(_)) {
            self.apply_conversion(Conversion::Double, &value)?
        } else {
            value
        };
        match operator {
            U::Not => self.logical_complement(&value),
            U::Minus => numeric::promoted_negate(&value),
            U::Plus => {
                if matches!(value, Value::Null | Value::DateTime(_))
                    || numeric::fixed(&value).is_some()
                {
                    Ok(value)
                } else {
                    numeric::float(&value)?;
                    Ok(value)
                }
            }
        }
    }
}

#[cfg(test)]
mod retained_tests;
