//! Provider-independent admission and metered String Let-coercion for LIKE.
use super::{EvalError, Evaluator, ExpressionValue, Operand, PreparedLikeOperands, Value};
use crate::functions::text::TextFunction;

impl Evaluator<'_> {
    /// Prepare retained operands before any host comparison-provider lookup.
    /// The explicit wildcard mode is still required for NULL. Otherwise NULL
    /// precedes conversion, including invalid or unrepresentable peer data.
    pub fn prepare_expression_like<'a>(
        &mut self,
        value: &'a ExpressionValue,
        pattern: &'a ExpressionValue,
    ) -> Result<PreparedLikeOperands<'a>, EvalError> {
        self.begin_like()?;
        if matches!(value, ExpressionValue::Value(Value::Null))
            || matches!(pattern, ExpressionValue::Value(Value::Null))
        {
            return Ok(PreparedLikeOperands { operands: None });
        }

        let value = self.prepare_like_expression_operand(value)?;
        let pattern = self.prepare_like_expression_operand(pattern)?;
        Ok(PreparedLikeOperands {
            operands: Some((value, pattern)),
        })
    }

    pub(in crate::evaluation) fn prepare_scalar_like<'a>(
        &mut self,
        value: &'a Value,
        pattern: &'a Value,
    ) -> Result<PreparedLikeOperands<'a>, EvalError> {
        self.begin_like()?;
        if matches!(value, Value::Null) || matches!(pattern, Value::Null) {
            return Ok(PreparedLikeOperands { operands: None });
        }

        let value = self.prepare_like_scalar_operand(value)?;
        let pattern = self.prepare_like_scalar_operand(pattern)?;
        Ok(PreparedLikeOperands {
            operands: Some((value, pattern)),
        })
    }

    fn begin_like(&mut self) -> Result<(), EvalError> {
        self.check(0)?;
        self.like_mode.ok_or(EvalError::LikeModeRequired)?;
        Ok(())
    }

    fn prepare_like_expression_operand<'a>(
        &mut self,
        value: &'a ExpressionValue,
    ) -> Result<Operand<'a>, EvalError> {
        match value {
            ExpressionValue::Value(value) => self.prepare_like_scalar_operand(value),
            ExpressionValue::StringBytes(value) => Ok(Operand::Raw(value)),
            ExpressionValue::Empty => self.text("").map(Operand::Converted),
            ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
            ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        }
    }

    fn prepare_like_scalar_operand<'a>(
        &mut self,
        value: &'a Value,
    ) -> Result<Operand<'a>, EvalError> {
        match value {
            Value::Text(text) => {
                if text.len() > self.limits.max_text_bytes {
                    return Err(EvalError::TextLimit);
                }
                Ok(Operand::Scalar(value))
            }
            Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_) => self
                .apply_text_function(TextFunction::CStr, std::slice::from_ref(value))
                .map(Operand::Converted),
            _ => Err(EvalError::TypeMismatch),
        }
    }
}
