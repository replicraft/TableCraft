//! Closed, allocation-free numeric functions over explicit scalar carriers.
use crate::EvalError;
use tablecraft_core::values::Value;

mod rounding;
mod unary;

/// Numeric built-ins; no locale parsing, host calls or implicit date conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathFunction {
    /// Absolute magnitude with checked same-carrier results.
    Abs,
    /// Sign as an Access Integer (-1, 0, 1).
    Sgn,
    /// Greatest integral value no larger than the operand.
    Int,
    /// Integral portion truncated toward zero.
    Fix,
    /// Half-even rounding with an optional decimal-place count.
    Round,
}
impl MathFunction {
    /// Maximum number of evaluated arguments retained on the stack.
    pub const MAX_ARGUMENTS: usize = 2;
    /// Allocation-free, ASCII-insensitive lookup.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Abs", Self::Abs),
            ("Sgn", Self::Sgn),
            ("Int", Self::Int),
            ("Fix", Self::Fix),
            ("Round", Self::Round),
        ]
        .into_iter()
        .find_map(|(spelling, function)| name.eq_ignore_ascii_case(spelling).then_some(function))
    }
    /// Whether a complete argument list is supported. Missing slots are rejected.
    pub const fn accepts_arity(self, count: usize) -> bool {
        count == 1 || (matches!(self, Self::Round) && count == Self::MAX_ARGUMENTS)
    }
    /// Apply already evaluated numeric arguments. Callers evaluate every supplied
    /// argument in written order, including when the first is NULL. Results retain
    /// carriers except Sgn -> Int16 and Boolean operands -> Int16 numeric values.
    /// Round validates a supplied place count before propagating NULL. Non-finite
    /// inputs, carrier overflow and unsupported nonnumeric values fail explicitly.
    pub fn apply(self, arguments: &[Value]) -> Result<Value, EvalError> {
        if !self.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        if matches!(self, Self::Round) {
            return rounding::apply(&arguments[0], arguments.get(1));
        }
        unary::apply(self, &arguments[0])
    }
}
#[cfg(test)]
mod tests;
