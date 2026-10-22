//! Wider exact totals for means; source carriers and SUM retain their own rules.
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{self, Arithmetic},
};

pub(super) struct Mean {
    total: Value,
    decimal: bool,
    floating: bool,
}
impl Default for Mean {
    fn default() -> Self {
        Self {
            total: Value::Int64(0),
            decimal: false,
            floating: false,
        }
    }
}
impl Mean {
    // The caller omits NULL, meters each record and checks cancellation.
    pub(super) fn push(&mut self, value: &Value) -> Result<(), EvalError> {
        match value {
            Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Currency(_) => {}
            Value::Decimal(_) => self.decimal = true,
            Value::Single(_) | Value::Double(_) => self.floating = true,
            _ => return Err(EvalError::TypeMismatch),
        }
        // Integral/Currency totals need more width than their source carriers.
        // Explicit Decimal retains Decimal precedence; otherwise a floating
        // operand moves the total into the existing finite Double profile.
        let conversion = if self.decimal || !self.floating {
            Conversion::Decimal
        } else {
            Conversion::Double
        };
        let total = conversion.apply(&self.total)?;
        let value = conversion.apply(value)?;
        self.total = numeric::arithmetic(Arithmetic::Add, &total, &value)?;
        Ok(())
    }
    pub(super) fn finish(self, count: i64) -> Result<Value, EvalError> {
        if count == 0 {
            return Ok(Value::Null);
        }
        let mean =
            numeric::promoted_arithmetic(Arithmetic::Divide, &self.total, &Value::Int64(count))?;
        if self.decimal {
            Ok(mean)
        } else {
            Conversion::Double.apply(&mean)
        }
    }
}
