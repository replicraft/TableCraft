//! Declaration-aware quotient/remainder after checked contextual operand rounding.
use super::Evaluator;
use crate::{
    EvalError,
    numeric::{self, Arithmetic, ArithmeticDeclarations, admission},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Apply integer division or Mod using original declaration categories and
    /// the existing rounded integral operand profile. Numeric Text requires this
    /// evaluator's explicit NumberFormat; legacy dates use their numeric serial.
    /// NULL follows contextual arithmetic propagation, after operand evaluation
    /// by the caller. This does not infer native SQL declarations.
    pub fn apply_declared_rounded_arithmetic(
        &mut self,
        operator: Arithmetic,
        left: &Value,
        right: &Value,
        declarations: ArithmeticDeclarations,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        self.rounded_binary_with(
            operator,
            left,
            right,
            Value::Null,
            |operator, left, right| {
                numeric::declared_integral_arithmetic(operator, left, right, declarations)
            },
        )
    }

    pub(super) fn rounded_binary_with<T>(
        &mut self,
        operator: Arithmetic,
        left: &Value,
        right: &Value,
        null: T,
        compute: impl FnOnce(Arithmetic, &Value, &Value) -> Result<T, EvalError>,
    ) -> Result<T, EvalError> {
        if !matches!(operator, Arithmetic::IntegerDivide | Arithmetic::Modulo) {
            return Err(EvalError::UnsupportedExpression);
        }

        for operand in [left, right] {
            admission::contextual(operand)?;
        }

        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(null);
        }
        let target = numeric::integer_operand_conversion(left, right)?;
        let left = self.apply_conversion(target, left)?;
        let right = self.apply_conversion(target, right)?;
        compute(operator, &left, &right)
    }
}

#[cfg(test)]
mod tests;
