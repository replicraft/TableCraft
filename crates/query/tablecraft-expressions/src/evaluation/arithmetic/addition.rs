//! Raw strings select concatenation without bypassing numeric declaration rules.
use super::Evaluator;
use crate::evaluation::arithmetic_operands;
use crate::{
    EvalError,
    functions::{
        conversions::errors::into_operator_scalar,
        text::byte_string::{ByteOperand, ExpressionByteOperand},
    },
    numeric::admission,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_sql::select::{BinaryOperator, Expression};

/// A completed string/NULL addition or materialized operands for the caller's
/// existing numeric/declared dispatch. This does not establish declarations.
pub enum PreparedAddition {
    /// A string concatenation or NULL result that needs no numeric dispatch.
    StringResult(ByteOperand),
    /// Materialized values retaining the caller's original declaration categories.
    ScalarOperands {
        /// Left operand in written order.
        left: Value,
        /// Right operand in written order.
        right: Value,
    },
}

impl Evaluator<'_> {
    pub(in crate::evaluation) fn addition_call(
        &mut self,
        left: &Expression,
        right: &Expression,
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        let left = self.expression_byte_operand(left, depth + 1)?;
        let right = self.expression_byte_operand(right, depth + 1)?;
        match self.prepare_expression_addition(left, right)? {
            PreparedAddition::StringResult(value) => Ok(value),
            PreparedAddition::ScalarOperands { left, right } => self
                .apply_binary(BinaryOperator::Add, &left, &right)
                .map(ByteOperand::Scalar),
        }
    }

    /// Admit already evaluated operands without prematurely publishing raw
    /// strings. Ordinary numeric operands retain their values and caller-owned
    /// provenance; host Empty follows the operator-specific admission profile.
    pub fn prepare_expression_addition(
        &mut self,
        left: ExpressionByteOperand,
        right: ExpressionByteOperand,
    ) -> Result<PreparedAddition, EvalError> {
        let (left, right) = match (left, right) {
            (
                ExpressionByteOperand::Scalar(ExpressionValue::Empty),
                ExpressionByteOperand::Bytes(bytes),
            )
            | (
                ExpressionByteOperand::Bytes(bytes),
                ExpressionByteOperand::Scalar(ExpressionValue::Empty),
            ) => {
                self.check(0)?;
                return Ok(PreparedAddition::StringResult(ByteOperand::Bytes(bytes)));
            }
            (
                ExpressionByteOperand::Scalar(ExpressionValue::Empty),
                ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Text(text))),
            )
            | (
                ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Text(text))),
                ExpressionByteOperand::Scalar(ExpressionValue::Empty),
            ) => {
                self.check(0)?;
                if text.len() > self.limits.max_text_bytes {
                    return Err(EvalError::TextLimit);
                }
                return Ok(PreparedAddition::StringResult(ByteOperand::Scalar(
                    Value::Text(text),
                )));
            }
            (ExpressionByteOperand::Scalar(left), ExpressionByteOperand::Scalar(right)) => {
                let (left, right) = arithmetic_operands(BinaryOperator::Add, left, right)?;
                return Ok(PreparedAddition::ScalarOperands { left, right });
            }
            operands => operands,
        };
        let left = admit(left)?;
        let right = admit(right)?;

        self.check(0)?;
        for operand in [&left, &right] {
            if let ByteOperand::Scalar(value) = operand {
                admission::contextual(value)?;
            }
        }
        if matches!(left, ByteOperand::Scalar(Value::Null))
            || matches!(right, ByteOperand::Scalar(Value::Null))
        {
            return Ok(PreparedAddition::StringResult(ByteOperand::Scalar(
                Value::Null,
            )));
        }

        let string = |operand: &ByteOperand| {
            matches!(
                operand,
                ByteOperand::Bytes(_) | ByteOperand::Scalar(Value::Text(_))
            )
        };
        if string(&left) && string(&right) {
            return self
                .apply_expression_concatenation(left.into(), right.into())
                .map(PreparedAddition::StringResult);
        }

        // Mixed String/numeric addition still needs ordinary numeric Text.
        // Strict publication refuses unpaired/odd data instead of replacing it.
        let left = self.publish_byte_operand(left)?;
        let right = self.publish_byte_operand(right)?;
        Ok(PreparedAddition::ScalarOperands { left, right })
    }
}

fn admit(operand: ExpressionByteOperand) -> Result<ByteOperand, EvalError> {
    match operand {
        ExpressionByteOperand::Scalar(value) => {
            into_operator_scalar(value).map(ByteOperand::Scalar)
        }
        ExpressionByteOperand::Bytes(bytes) => Ok(ByteOperand::Bytes(bytes)),
    }
}

#[cfg(test)]
mod tests;
