//! Contextual logical Text inputs share conversion policy and cumulative budgets.
use super::Evaluator;
use crate::{
    EvalError,
    functions::conversions::Conversion,
    logic::{self, Logical, Truth},
};
use tablecraft_core::values::Value;

mod operands;

impl Evaluator<'_> {
    pub(super) fn apply_logical(
        &mut self,
        operator: Logical,
        left: &Value,
        right: &Value,
    ) -> Result<Value, EvalError> {
        if !matches!(left, Value::Text(_)) && !matches!(right, Value::Text(_)) {
            return operator.apply(left, right);
        }
        // Select the result width from original operands, before Text conversion
        // can discard the fact that the other operand was already Int64.
        let conversion =
            logic::binary_conversion(left, right).ok_or(EvalError::UnsupportedExpression)?;
        let left = self.logical_operand(left, conversion)?;
        let right = self.logical_operand(right, conversion)?;
        operator.apply(&left, &right)
    }

    fn logical_operand(
        &mut self,
        value: &Value,
        conversion: Conversion,
    ) -> Result<Value, EvalError> {
        if matches!(value, Value::Null) {
            return Ok(Value::Null);
        }
        if matches!(value, Value::Text(_)) {
            self.check(0)?;
        }
        self.apply_conversion(conversion, value)
    }

    pub(super) fn logical_complement(&mut self, value: &Value) -> Result<Value, EvalError> {
        if matches!(value, Value::Text(_)) {
            let value = self.logical_operand(value, Conversion::Long)?;
            logic::complement(&value)
        } else {
            logic::complement(value)
        }
    }

    /// Interpret a final read predicate under execution-local policies. Numeric
    /// values retain unrounded zero/nonzero truth; Text uses explicit CBool
    /// conversion and its cumulative work/payload budget. NULL remains unknown.
    pub fn predicate_truth(&mut self, value: &Value) -> Result<Truth, EvalError> {
        if matches!(value, Value::Text(_)) {
            self.check(0)?;
            let value = self.apply_conversion(Conversion::Boolean, value)?;
            Truth::from_value(&value)
        } else {
            Truth::from_predicate(value)
        }
    }
}

#[cfg(test)]
mod tests;
