//! Declaration-aware quotient/remainder after checked contextual operand rounding.
use super::Evaluator;
use crate::{
    EvalError,
    numeric::{self, Arithmetic, ArithmeticDeclarations},
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
        if !matches!(operator, Arithmetic::IntegerDivide | Arithmetic::Modulo) {
            return Err(EvalError::UnsupportedExpression);
        }
        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(Value::Null);
        }
        let target = numeric::integer_operand_conversion(left, right)?;
        let left = self.apply_conversion(target, left)?;
        let right = self.apply_conversion(target, right)?;
        numeric::declared_integral_arithmetic(operator, &left, &right, declarations)
    }
}

#[cfg(test)]
mod tests;
