//! Closed, allocation-free numeric functions over explicit scalar carriers.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

mod colors;
pub mod random;
mod rounding;
mod transcendental;
mod unary;

/// Numeric built-ins with explicit carrier and legacy DATE conversion contracts.
/// Rnd additionally requires an explicit caller-owned sequence.
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
    /// Nonnegative square root as Double.
    Sqr,
    /// Natural exponential as Double.
    Exp,
    /// Positive-domain natural logarithm as Double.
    Log,
    /// Sine of a radian angle as Double.
    Sin,
    /// Cosine of a radian angle as Double.
    Cos,
    /// Tangent of a radian angle as Double.
    Tan,
    /// Principal arctangent in radians as Double.
    Atn,
    /// Half-even rounding with an optional decimal-place count.
    Round,
    /// Pack numeric red/green/blue channels into an Access Long.
    Rgb,
    /// Look up an Access Long in the sixteen-color Basic palette.
    QbColor,
    /// Draw from an explicit caller-owned Basic pseudo-random sequence.
    Rnd,
}

impl MathFunction {
    /// Maximum number of evaluated arguments retained on the stack.
    pub const MAX_ARGUMENTS: usize = 3;
    /// Required channel count for RGB.
    pub const RGB_ARGUMENTS: usize = Self::MAX_ARGUMENTS;
    /// Maximum Round width: value and optional decimal-place count.
    pub const ROUND_ARGUMENTS: usize = 2;
    /// Allocation-free, ASCII-insensitive lookup.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Abs", Self::Abs),
            ("Sgn", Self::Sgn),
            ("Int", Self::Int),
            ("Fix", Self::Fix),
            ("Round", Self::Round),
            ("Sqr", Self::Sqr),
            ("Exp", Self::Exp),
            ("Log", Self::Log),
            ("Sin", Self::Sin),
            ("Cos", Self::Cos),
            ("Tan", Self::Tan),
            ("Atn", Self::Atn),
            ("RGB", Self::Rgb),
            ("QBColor", Self::QbColor),
            ("Rnd", Self::Rnd),
        ]
        .into_iter()
        .find_map(|(spelling, function)| name.eq_ignore_ascii_case(spelling).then_some(function))
    }

    /// Whether a complete argument list is supported. Missing slots are rejected.
    pub const fn accepts_arity(self, count: usize) -> bool {
        match self {
            Self::Rnd => count <= 1,
            Self::Rgb => count == Self::RGB_ARGUMENTS,
            Self::Round => count == 1 || count == Self::ROUND_ARGUMENTS,
            _ => count == 1,
        }
    }

    /// Apply already evaluated numeric arguments. Callers evaluate every supplied
    /// argument in written order, including when the first is NULL. Abs/Int/Fix/
    /// Round retain carriers, with Boolean operands becoming Int16. Sgn -> Int16.
    /// Sqr/Exp/Log/Sin/Cos/Tan/Atn explicitly convert numeric operands to Double.
    /// Round validates a supplied place count before propagating NULL. Non-finite
    /// inputs, carrier overflow and unsupported nonnumeric values fail explicitly.
    /// RGB uses checked Long channels; QBColor checks an Integer palette index.
    /// Both return a packed Long; NULL conversion fails.
    /// Rnd requires apply_with_random; no implicit default state is constructed.
    pub fn apply(self, arguments: &[Value]) -> Result<Value, EvalError> {
        self.apply_with_random(arguments, None)
    }

    /// Apply numeric functions with an explicit sequence available only to Rnd.
    pub fn apply_with_random(
        self,
        arguments: &[Value],
        random: Option<&random::RndState>,
    ) -> Result<Value, EvalError> {
        self.apply_with_input(
            arguments.len(),
            &mut |index| Ok(&arguments[index]),
            random,
            &mut |target, value| target.apply(value),
        )
    }

    pub(crate) fn apply_with_input<'a>(
        self,
        count: usize,
        argument: &mut impl FnMut(usize) -> Result<&'a Value, EvalError>,
        random: Option<&random::RndState>,
        convert: &mut impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
    ) -> Result<Value, EvalError> {
        if !self.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        if self == Self::Rnd {
            let random = random.ok_or(EvalError::RandomStateRequired)?;
            let number = if count == 0 { None } else { Some(argument(0)?) };
            return random.apply_with_converter(number, convert);
        }
        if matches!(self, Self::Rgb | Self::QbColor) {
            return colors::apply_with_input(self, argument, convert);
        }
        if matches!(self, Self::Round) {
            return rounding::apply_with_input(count, argument, convert);
        }
        let value = argument(0)?;
        let converted = if matches!(value, Value::Text(_)) {
            Some(convert(Conversion::Double, value)?)
        } else {
            None
        };
        let value = converted.as_ref().unwrap_or(value);
        if matches!(
            self,
            Self::Sqr | Self::Exp | Self::Log | Self::Sin | Self::Cos | Self::Tan | Self::Atn
        ) {
            return transcendental::apply(self, value);
        }
        unary::apply(self, value)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod date_tests;
