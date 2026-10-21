//! Fixed-rate annuities over finite numeric carriers, without host/runtime state.
mod annuity;
mod payments;
mod periods;
#[cfg(feature = "sql")]
pub mod sql_arguments;
use crate::{EvalError, functions::conversions::Conversion};
use annuity::Annuity;
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
    /// Number of payment periods required by a fixed-rate cashflow equation.
    NPer,
    /// Interest portion of one payment period.
    IPmt,
    /// Principal portion of one payment period.
    PPmt,
}
impl FinancialFunction {
    /// Minimum required arguments among the supported financial functions.
    pub const REQUIRED_ARGUMENTS: usize = 3;
    /// Maximum supported call width, including per, balance and timing.
    pub const MAX_ARGUMENTS: usize = 6;
    /// ASCII-insensitive lookup; never resolves application functions.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Pmt", Self::Pmt),
            ("PV", Self::PV),
            ("FV", Self::FV),
            ("NPer", Self::NPer),
            ("IPmt", Self::IPmt),
            ("PPmt", Self::PPmt),
        ]
        .into_iter()
        .find_map(|(label, function)| name.eq_ignore_ascii_case(label).then_some(function))
    }
    /// Required arguments before the optional balance and timing positions.
    pub const fn required_arguments(self) -> usize {
        if matches!(self, Self::IPmt | Self::PPmt) {
            4
        } else {
            Self::REQUIRED_ARGUMENTS
        }
    }
    /// Whether the supplied argument count is supported.
    pub const fn accepts_arity(self, count: usize) -> bool {
        count >= self.required_arguments() && count <= self.required_arguments() + 2
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
        let result = match self {
            Self::IPmt | Self::PPmt => {
                let [rate, per, periods, present, future, timing] = numbers;
                check_timing(timing)?;
                payments::calculate(self, rate, per, periods, present, future, timing)?
            }
            _ => {
                let [rate, second, cash, balance, timing, _] = numbers;
                check_timing(timing)?;
                match self {
                    Self::NPer => periods::calculate(rate, second, cash, balance, timing)?,
                    Self::Pmt => {
                        annuity::calculate(Annuity::Payment, rate, second, cash, balance, timing)?
                    }
                    Self::PV => {
                        annuity::calculate(Annuity::Present, rate, second, cash, balance, timing)?
                    }
                    Self::FV => {
                        annuity::calculate(Annuity::Future, rate, second, cash, balance, timing)?
                    }
                    _ => return Err(EvalError::UnsupportedExpression),
                }
            }
        };
        if !result.is_finite() {
            return Err(EvalError::Overflow);
        }
        Ok(Value::Double(Double::from_float(result)))
    }
}
#[cfg(test)]
mod tests;

fn check_timing(timing: f64) -> Result<(), EvalError> {
    if timing != 0.0 && timing != 1.0 {
        Err(EvalError::Domain)
    } else {
        Ok(())
    }
}
