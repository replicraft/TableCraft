//! Numeric arithmetic admits raw String families before any Unicode conversion.
use super::Evaluator;
use crate::{
    EvalError,
    evaluation::{arithmetic_operands, empty_division_operand},
    functions::text::byte_string::{ByteOperand, ExpressionByteOperand},
    numeric::admission,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_sql::select::{BinaryOperator as B, Expression};

/// A completed result or admitted scalar operands for the caller's existing
/// declaration-aware numeric dispatch. No declaration is inferred here.
pub enum PreparedNumericArithmetic {
    /// A declaration-independent result, currently NULL propagation.
    Result(Value),
    /// Original scalar carriers, with raw Strings strictly published as Text.
    Operands {
        /// Left operand in written order.
        left: Value,
        /// Right operand in written order.
        right: Value,
    },
}

impl Evaluator<'_> {
    pub(in crate::evaluation) fn numeric_arithmetic_call(
        &mut self,
        operator: B,
        left: &Expression,
        right: &Expression,
        depth: usize,
    ) -> Result<Value, EvalError> {
        let left = self.expression_byte_operand(left, depth + 1)?;
        let right = self.expression_byte_operand(right, depth + 1)?;

        match self.prepare_expression_numeric_arithmetic(operator, left, right)? {
            PreparedNumericArithmetic::Result(value) => Ok(value),
            PreparedNumericArithmetic::Operands { left, right } => {
                self.apply_binary(operator, &left, &right)
            }
        }
    }

    /// Admit eager operands for the six non-addition numeric operators. Raw
    /// Strings can propagate NULL without decoding; non-NULL operands must
    /// publish valid Text before existing numeric conversion/dispatch. Overloaded
    /// addition retains its separate preparation service.
    pub fn prepare_expression_numeric_arithmetic(
        &mut self,
        operator: B,
        left: ExpressionByteOperand,
        right: ExpressionByteOperand,
    ) -> Result<PreparedNumericArithmetic, EvalError> {
        if !matches!(
            operator,
            B::Subtract | B::Multiply | B::Divide | B::IntegerDivide | B::Modulo | B::Power
        ) {
            return Err(EvalError::UnsupportedExpression);
        }

        let (left, right) = match (left, right) {
            (ExpressionByteOperand::Scalar(left), ExpressionByteOperand::Scalar(right)) => {
                return self.numeric_arithmetic_inputs(operator, left, right);
            }
            operands => operands,
        };

        admit(&left)?;
        admit(&right)?;
        self.check(0)?;
        if left.is_null() || right.is_null() {
            return Ok(PreparedNumericArithmetic::Result(Value::Null));
        }

        let left = self.numeric_string_operand(left)?;
        let right = self.numeric_string_operand(right)?;
        self.numeric_arithmetic_inputs(operator, left, right)
    }

    fn numeric_arithmetic_inputs(
        &mut self,
        operator: B,
        left: ExpressionValue,
        right: ExpressionValue,
    ) -> Result<PreparedNumericArithmetic, EvalError> {
        if operator == B::Divide
            && let Some(numerator) = empty_division_operand(&left, &right)
        {
            return self
                .apply_empty_division(numerator)
                .map(PreparedNumericArithmetic::Result);
        }

        let (left, right) = arithmetic_operands(operator, left, right)?;
        Ok(PreparedNumericArithmetic::Operands { left, right })
    }

    fn numeric_string_operand(
        &mut self,
        operand: ExpressionByteOperand,
    ) -> Result<ExpressionValue, EvalError> {
        match operand {
            ExpressionByteOperand::Scalar(value) => Ok(value),
            ExpressionByteOperand::Bytes(bytes) => self
                .publish_byte_operand(ByteOperand::Bytes(bytes))
                .map(ExpressionValue::Value),
        }
    }
}

fn admit(operand: &ExpressionByteOperand) -> Result<(), EvalError> {
    match operand {
        ExpressionByteOperand::Scalar(ExpressionValue::Value(value)) => {
            admission::contextual(value)
        }
        ExpressionByteOperand::Scalar(ExpressionValue::Error(_)) => Err(EvalError::TypeMismatch),
        ExpressionByteOperand::Scalar(ExpressionValue::Missing) => {
            Err(EvalError::NonScalarExpression)
        }
        ExpressionByteOperand::Scalar(ExpressionValue::Empty) | ExpressionByteOperand::Bytes(_) => {
            Ok(())
        }
    }
}
