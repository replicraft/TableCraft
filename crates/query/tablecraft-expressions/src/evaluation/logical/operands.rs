//! Logical operands retain expression identity until both written inputs exist.
use super::{Evaluator, Logical};
use crate::{
    EvalError,
    functions::conversions::{Conversion, errors::into_operator_scalar},
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Apply a logical operator to eager expression data. Empty contributes an
    /// Integer zero; raw Strings use strict Unicode publication and the existing
    /// contextual numeric conversion. NULL never skips conversion of its peer.
    /// Hosts must evaluate both written expressions before calling this method.
    pub fn apply_expression_logical(
        &mut self,
        operator: Logical,
        left: ExpressionValue,
        right: ExpressionValue,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        admit(&left)?;
        admit(&right)?;

        let raw = matches!(left, ExpressionValue::StringBytes(_))
            || matches!(right, ExpressionValue::StringBytes(_));
        if raw {
            // Choose width from original operands before publishing Strings.
            // Complete each written conversion before starting the next one,
            // so a left overflow/policy failure precedes a bad right String.
            let target = if matches!(left, ExpressionValue::Value(Value::Int64(_)))
                || matches!(right, ExpressionValue::Value(Value::Int64(_)))
            {
                Conversion::LongLong
            } else {
                Conversion::Long
            };
            let left = self.logical_expression_operand(left)?;
            let left = self.logical_operand(&left, target)?;
            let right = self.logical_expression_operand(right)?;
            let right = self.logical_operand(&right, target)?;
            return operator.apply(&left, &right);
        }

        let left = self.logical_expression_operand(left)?;
        let right = self.logical_expression_operand(right)?;
        self.apply_logical(operator, &left, &right)
    }

    /// Complement expression data using the same Empty/String admission as
    /// binary logical operators. Empty produces Integer -1, not Boolean True.
    pub fn apply_expression_complement(
        &mut self,
        value: ExpressionValue,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        admit(&value)?;

        let value = self.logical_expression_operand(value)?;
        self.logical_complement(&value)
    }

    fn logical_expression_operand(&mut self, value: ExpressionValue) -> Result<Value, EvalError> {
        match value {
            ExpressionValue::Empty => Ok(Value::Int16(0)),
            ExpressionValue::StringBytes(bytes) => self.publish_expression_string(&bytes),
            value => into_operator_scalar(value),
        }
    }
}

fn admit(value: &ExpressionValue) -> Result<(), EvalError> {
    match value {
        ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
        ExpressionValue::Missing => Err(EvalError::NonScalarExpression),
        ExpressionValue::Value(_) | ExpressionValue::Empty | ExpressionValue::StringBytes(_) => {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
