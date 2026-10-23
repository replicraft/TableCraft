//! Contextual numeric Text operands; standalone numeric kernels stay policy-free.
mod rounded;
use super::Evaluator;
use crate::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{self, Arithmetic, ArithmeticDeclarations},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Apply numeric selected-width overflow with caller-issued declarations.
    /// This meters cancellation/work before the allocation-free numeric kernel.
    pub fn apply_declared_numeric_arithmetic(
        &mut self,
        operator: Arithmetic,
        left: &Value,
        right: &Value,
        declarations: ArithmeticDeclarations,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        numeric::declared_numeric_arithmetic(operator, left, right, declarations)
    }

    /// Apply nullable integral arithmetic with caller-established declarations.
    /// This checks cancellation/shared work before the allocation-free kernel;
    /// it does not infer declarations or change default SQL operator dispatch.
    pub fn apply_declared_integral_arithmetic(
        &mut self,
        operator: Arithmetic,
        left: &Value,
        right: &Value,
        declarations: ArithmeticDeclarations,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        numeric::declared_integral_arithmetic(operator, left, right, declarations)
    }

    pub(super) fn numeric_binary(
        &mut self,
        operator: Arithmetic,
        left: &Value,
        right: &Value,
    ) -> Result<Value, EvalError> {
        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(Value::Null);
        }
        let text = matches!(left, Value::Text(_)) || matches!(right, Value::Text(_));
        let decimal = matches!(left, Value::Decimal(_)) || matches!(right, Value::Decimal(_));
        let currency = matches!(left, Value::Currency(_)) || matches!(right, Value::Currency(_));
        let date = matches!(left, Value::DateTime(_)) || matches!(right, Value::DateTime(_));
        let serial = date && matches!(operator, Arithmetic::Add | Arithmetic::Subtract);
        // Text must not pass through binary floating point on its way to Decimal
        // or rounded integer division. Currency addition/subtraction round at the
        // existing checked four-place conversion boundary.
        let target = match operator {
            Arithmetic::Power => Conversion::Double,
            Arithmetic::IntegerDivide | Arithmetic::Modulo => {
                numeric::integer_operand_conversion(left, right)?
            }
            _ if decimal => Conversion::Decimal,
            Arithmetic::Add | Arithmetic::Subtract if currency && !date => Conversion::Currency,
            _ => Conversion::Double,
        };
        let promote = text
            && !serial
            && (operator == Arithmetic::Power
                || (currency && !decimal && target == Conversion::Double)
                || date);
        let converted_left = if text && (matches!(left, Value::Text(_)) || promote) {
            Some(self.apply_conversion(target, left)?)
        } else {
            None
        };
        let converted_right = if text && (matches!(right, Value::Text(_)) || promote) {
            Some(self.apply_conversion(target, right)?)
        } else {
            None
        };
        let left = converted_left.as_ref().unwrap_or(left);
        let right = converted_right.as_ref().unwrap_or(right);
        numeric::promoted_arithmetic(operator, left, right)
    }
}

#[cfg(test)]
mod tests;
