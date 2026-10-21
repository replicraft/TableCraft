//! Fixed-rate annuities over finite numeric carriers, without host/runtime state.
mod annuity;
#[cfg(feature = "sql")]
pub mod sql_arguments;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{Double, Value};

/// Closed scalar financial operations; results are Double, not stored Currency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinancialFunction {
    /// Periodic payment from present and future balances.
    Pmt,
    /// Present balance from payments and a future balance.
    PV,
    /// Future balance from payments and a present balance.
    FV,
}
impl FinancialFunction {
    /// Required rate, periods and payment/present-value arguments.
    pub const REQUIRED_ARGUMENTS: usize = 3;
    /// Optional balance and payment timing bring the maximum to five.
    pub const MAX_ARGUMENTS: usize = 5;
    /// ASCII-insensitive lookup; never resolves application functions.
    pub fn resolve(name: &str) -> Option<Self> {
        [("Pmt", Self::Pmt), ("PV", Self::PV), ("FV", Self::FV)]
            .into_iter()
            .find_map(|(label, function)| name.eq_ignore_ascii_case(label).then_some(function))
    }
    /// Whether the supplied argument count is supported.
    pub const fn accepts_arity(self, count: usize) -> bool {
        count >= Self::REQUIRED_ARGUMENTS && count <= Self::MAX_ARGUMENTS
    }
    /// Apply already evaluated operands in written order using the existing Double
    /// conversion kernel. NULL and nonnumeric carriers fail rather than becoming
    /// zero. Omitted balance/timing default to zero. Timing is exactly 0 or 1.
    /// Callers evaluate every supplied expression before invoking this method.
    pub fn apply(self, arguments: &[Value]) -> Result<Value, EvalError> {
        if !self.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        let mut numbers = [0.0; Self::MAX_ARGUMENTS];
        for (argument, number) in arguments.iter().zip(&mut numbers) {
            let Value::Double(value) = Conversion::Double.apply(argument)? else {
                return Err(EvalError::TypeMismatch);
            };
            *number = value.to_float();
        }
        let [rate, periods, cash, balance, timing] = numbers;
        if timing != 0.0 && timing != 1.0 {
            return Err(EvalError::Domain);
        }
        let result = annuity::calculate(self, rate, periods, cash, balance, timing)?;
        if !result.is_finite() {
            return Err(EvalError::Overflow);
        }
        Ok(Value::Double(Double::from_float(result)))
    }
}
#[cfg(test)]
mod tests;
