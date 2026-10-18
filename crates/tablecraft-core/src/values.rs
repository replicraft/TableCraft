//! Exact scalar carriers. Equality compares representation, not Access SQL values.
//!
//! Field constraints, collations, coercions, codecs and arithmetic promotion are
//! separate responsibilities. Text is decoded Unicode; native source bytes and
//! encoding metadata must be preserved by the backend, not reconstructed here.

use std::fmt;

/// An expression-only state that must not silently become a stored NULL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpressionState {
    /// An uninitialized language value.
    Empty,
    /// An omitted optional argument.
    Missing,
    /// An expression error value.
    Error,
}

/// Closed scalar validation failures without the rejected data in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueError {
    /// Decimal coefficient exceeds the 28-digit Access field envelope.
    DecimalOutOfRange,
    /// Decimal scale is greater than 28.
    DecimalScaleOutOfRange,
    /// Requested rescaling would discard nonzero digits.
    DecimalPrecisionLoss,
    /// Currency arithmetic exceeded its signed 64-bit scaled range.
    CurrencyOverflow,
    /// Non-finite or out-of-calendar-range Automation DATE.
    InvalidLegacyDateTime,
    /// Invalid Gregorian date, time or fractional second.
    InvalidExtendedDateTime,
    /// An expression-only state cannot be converted to a stored scalar.
    ExpressionNotStorable(ExpressionState),
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DecimalOutOfRange => "decimal coefficient exceeds 28 digits",
            Self::DecimalScaleOutOfRange => "decimal scale must be between 0 and 28",
            Self::DecimalPrecisionLoss => "decimal rescaling would lose precision",
            Self::CurrencyOverflow => "currency arithmetic overflow",
            Self::InvalidLegacyDateTime => "invalid legacy date/time",
            Self::InvalidExtendedDateTime => "invalid extended date/time",
            Self::ExpressionNotStorable(_) => "expression-only state cannot be stored",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ValueError {}

/// Currency as an exact signed integer scaled by 10,000, with no currency symbol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Currency(i64);

impl Currency {
    /// Construct from units of one ten-thousandth, without float conversion.
    pub const fn from_scaled(scaled: i64) -> Self {
        Self(scaled)
    }

    /// Read units of one ten-thousandth.
    pub const fn scaled(self) -> i64 {
        self.0
    }

    /// Add exact Currency values, rejecting overflow rather than promoting.
    pub fn checked_add(self, rhs: Self) -> Result<Self, ValueError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(ValueError::CurrencyOverflow)
    }

    /// Subtract exact Currency values, rejecting overflow rather than wrapping.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, ValueError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(ValueError::CurrencyOverflow)
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_fixed(i128::from(self.0), 4, f)
    }
}

/// An exact Access field decimal: signed coefficient and retained scale 0..=28.
///
/// The coefficient has at most 28 decimal digits. A specific column can impose
/// smaller precision/scale limits, which its schema must check. This is not the
/// larger VBA Variant Decimal envelope (96-bit magnitude). No implicit rounding,
/// float conversion, scale normalization or arithmetic promotion is provided.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

impl Decimal {
    /// Largest absolute coefficient in the 28-digit field envelope.
    pub const MAX_COEFFICIENT: i128 = 10_i128.pow(28) - 1;

    /// Validate exact coefficient and scale without rounding or normalization.
    pub fn new(coefficient: i128, scale: u8) -> Result<Self, ValueError> {
        if scale > 28 {
            return Err(ValueError::DecimalScaleOutOfRange);
        }
        if !(-Self::MAX_COEFFICIENT..=Self::MAX_COEFFICIENT).contains(&coefficient) {
            return Err(ValueError::DecimalOutOfRange);
        }
        Ok(Self { coefficient, scale })
    }

    /// Return the exact signed coefficient.
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Return retained digits after the decimal point.
    pub const fn scale(self) -> u8 {
        self.scale
    }

    /// Change scale only when the result is exact and remains representable.
    pub fn rescale_exact(self, scale: u8) -> Result<Self, ValueError> {
        if scale > 28 {
            return Err(ValueError::DecimalScaleOutOfRange);
        }
        let coefficient = if scale >= self.scale {
            self.coefficient
                .checked_mul(10_i128.pow(u32::from(scale - self.scale)))
                .ok_or(ValueError::DecimalOutOfRange)?
        } else {
            let divisor = 10_i128.pow(u32::from(self.scale - scale));
            if self.coefficient % divisor != 0 {
                return Err(ValueError::DecimalPrecisionLoss);
            }
            self.coefficient / divisor
        };
        Self::new(coefficient, scale)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_fixed(self.coefficient, self.scale, f)
    }
}

fn display_fixed(coefficient: i128, scale: u8, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if coefficient < 0 {
        f.write_str("-")?;
    }
    let magnitude = coefficient.unsigned_abs();
    if scale == 0 {
        return write!(f, "{magnitude}");
    }
    let divisor = 10_u128.pow(u32::from(scale));
    write!(
        f,
        "{}.{:0width$}",
        magnitude / divisor,
        magnitude % divisor,
        width = usize::from(scale)
    )
}

macro_rules! float_value {
    ($name:ident, $bits:ty, $float:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name($bits);

        impl $name {
            /// Preserve all IEEE bits, including signed zero and NaN payloads.
            pub const fn from_bits(bits: $bits) -> Self {
                Self(bits)
            }

            /// Preserve the bit representation of a Rust float.
            pub const fn from_float(value: $float) -> Self {
                Self(value.to_bits())
            }

            /// Return the original bit representation.
            pub const fn to_bits(self) -> $bits {
                self.0
            }

            /// Read as a float; this does not define Access comparison semantics.
            pub const fn to_float(self) -> $float {
                <$float>::from_bits(self.0)
            }
        }
    };
}

float_value!(
    Single,
    u32,
    f32,
    "A 32-bit float carrier with bitwise equality, not SQL equality."
);
float_value!(
    Double,
    u64,
    f64,
    "A 64-bit float carrier with bitwise equality, not SQL equality."
);

/// A GUID's canonical 16-byte sequence in textual group order.
///
/// Native GUID struct byte ordering is a codec concern. This type supplies no
/// Access index ordering, UUID generation, or conversion from semantic IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Guid([u8; 16]);

impl Guid {
    /// Wrap canonical bytes; all 128-bit patterns are retained.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Return canonical bytes, not an MDB/ACCDB field encoding.
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// A timezone-free legacy Automation DATE preserving its exact double bits.
///
/// The signed truncated day is relative to 1899-12-30; the absolute fractional
/// part is time of day, including for negative values. No timestamp conversion
/// or numeric ordering is provided. Equality is representational, not temporal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LegacyDateTime(u64);

impl LegacyDateTime {
    /// First whole day in the year-100-through-9999 calendar envelope.
    pub const MIN_DAY: i32 = -657_434;
    /// Last whole day in the year-100-through-9999 calendar envelope.
    pub const MAX_DAY: i32 = 2_958_465;

    /// Validate an Automation DATE without changing its floating-point bits.
    pub fn from_ole_days(days: f64) -> Result<Self, ValueError> {
        if !days.is_finite()
            || !(f64::from(Self::MIN_DAY)..=f64::from(Self::MAX_DAY)).contains(&days.trunc())
        {
            return Err(ValueError::InvalidLegacyDateTime);
        }
        Ok(Self(days.to_bits()))
    }

    /// Validate a DATE from exact double bits.
    pub fn from_bits(bits: u64) -> Result<Self, ValueError> {
        Self::from_ole_days(f64::from_bits(bits))
    }

    /// Return the original double bits.
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    /// Return the original Automation DATE double, not a Unix timestamp.
    pub const fn ole_days(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Return the signed, truncated calendar-day offset from 1899-12-30.
    pub fn day_offset(self) -> i32 {
        self.ole_days().trunc() as i32
    }

    /// Return the nonnegative time-of-day fraction without quantizing it.
    pub fn fraction_of_day(self) -> f64 {
        self.ole_days().fract().abs()
    }
}

/// A Gregorian, timezone-free date/time with seven fractional-second digits.
///
/// Years are 1..=9999. A fractional unit is 100 ns. This logical carrier does
/// not implement Access's native 42-byte encoding or calendar conversions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExtendedDateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    fraction_100ns: u32,
}

impl ExtendedDateTime {
    /// Validate Gregorian calendar components and a 0..=9,999,999 fraction.
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        fraction_100ns: u32,
    ) -> Result<Self, ValueError> {
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => 0,
        };
        if !(1..=9999).contains(&year)
            || day == 0
            || day > days
            || hour > 23
            || minute > 59
            || second > 59
            || fraction_100ns > 9_999_999
        {
            return Err(ValueError::InvalidExtendedDateTime);
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            fraction_100ns,
        })
    }

    /// Return year, month and day without introducing a timezone.
    pub const fn date(self) -> (u16, u8, u8) {
        (self.year, self.month, self.day)
    }

    /// Return hour, minute, second and 100 ns units within that second.
    pub const fn time(self) -> (u8, u8, u8, u32) {
        (self.hour, self.minute, self.second, self.fraction_100ns)
    }
}

/// Materialized scalar values. Field behavior and source preservation are external.
///
/// Integer widths and float bit patterns remain distinct. Text and binary are
/// owned materializations, not unbounded streaming APIs. Callers must enforce
/// resource budgets before allocating them. `Eq` compares carriers, not SQL
/// equality, collation, coercion, or three-valued NULL logic. Debug redacts data.
#[derive(Clone, Eq, PartialEq)]
pub enum Value {
    /// SQL NULL, distinct from zero-length text and VBA Empty.
    Null,
    /// A logical boolean, without VBA numeric coercion.
    Boolean(bool),
    /// Unsigned 8-bit Access Byte.
    Byte(u8),
    /// Signed 16-bit Integer.
    Int16(i16),
    /// Signed 32-bit Long Integer.
    Int32(i32),
    /// Signed 64-bit Large Number, subject to backend capability checks.
    Int64(i64),
    /// Single-precision float bits.
    Single(Single),
    /// Double-precision float bits.
    Double(Double),
    /// Exact fixed-scale money.
    Currency(Currency),
    /// Exact decimal with retained scale.
    Decimal(Decimal),
    /// Decoded Unicode text; an empty string is not NULL.
    Text(String),
    /// Binary data without an assumed OLE, attachment or text interpretation.
    Binary(Vec<u8>),
    /// Canonical GUID value, not an internal object identity.
    Guid(Guid),
    /// Legacy timezone-free date/time.
    DateTime(LegacyDateTime),
    /// Extended timezone-free date/time.
    DateTimeExtended(ExtendedDateTime),
}

impl Value {
    /// Return a diagnostic-safe carrier name, not a native schema type code.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Boolean(_) => "Boolean",
            Self::Byte(_) => "Byte",
            Self::Int16(_) => "Int16",
            Self::Int32(_) => "Int32",
            Self::Int64(_) => "Int64",
            Self::Single(_) => "Single",
            Self::Double(_) => "Double",
            Self::Currency(_) => "Currency",
            Self::Decimal(_) => "Decimal",
            Self::Text(_) => "Text",
            Self::Binary(_) => "Binary",
            Self::Guid(_) => "Guid",
            Self::DateTime(_) => "DateTime",
            Self::DateTimeExtended(_) => "DateTimeExtended",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(self.kind()).field(&"[redacted]").finish()
    }
}

/// An expression carrier that cannot silently persist language-only states.
///
/// This is not a VBA interpreter. Error codes are opaque codes supplied by the
/// evaluator, without implementing CVErr conversion or exception handling.
#[derive(Clone, Eq, PartialEq)]
pub enum ExpressionValue {
    /// An ordinary scalar, including SQL NULL.
    Value(Value),
    /// Uninitialized language value.
    Empty,
    /// Omitted optional argument.
    Missing,
    /// Opaque evaluator error code.
    Error(u32),
}

impl TryFrom<ExpressionValue> for Value {
    type Error = ValueError;

    fn try_from(value: ExpressionValue) -> Result<Self, Self::Error> {
        match value {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::Empty => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Empty))
            }
            ExpressionValue::Missing => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Missing))
            }
            ExpressionValue::Error(_) => {
                Err(ValueError::ExpressionNotStorable(ExpressionState::Error))
            }
        }
    }
}

impl fmt::Debug for ExpressionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.debug_tuple("Value").field(value).finish(),
            Self::Empty => f.write_str("Empty"),
            Self::Missing => f.write_str("Missing"),
            Self::Error(_) => f.write_str("Error([redacted])"),
        }
    }
}
