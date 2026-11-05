//! Shared exact typed text input; no SQL parsing or implicit carrier selection.
mod hint;
use crate::{Error, display::Presentation};
pub use hint::InputHint;
use tablecraft_core::values::{Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::{
            Conversion,
            text::{NumberFormat, decimal_exact},
        },
        text::output::{NumericBuffer, TextOutput},
    },
};
use tablecraft_schema::ScalarType;

pub(crate) const MAX_INPUT_BYTES: usize = 1024 * 1024;

fn invalid() -> Error {
    Error::InvalidScalarInput
}

pub(crate) fn parse(
    kind: ScalarType,
    text: &str,
    is_null: bool,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::ScalarInputLimit);
    }
    output.work(1 + text.len() as u64)?;
    if is_null {
        return Ok(Value::Null);
    }
    Ok(match kind {
        ScalarType::Boolean => Value::Boolean(text.parse().map_err(|_| invalid())?),
        ScalarType::Byte => Value::Byte(text.parse().map_err(|_| invalid())?),
        ScalarType::Int16 => Value::Int16(text.parse().map_err(|_| invalid())?),
        ScalarType::Int32 => Value::Int32(text.parse().map_err(|_| invalid())?),
        ScalarType::Int64 => Value::Int64(text.parse().map_err(|_| invalid())?),
        ScalarType::Currency => convert(Conversion::Currency, text, output)?,
        ScalarType::Decimal => convert(Conversion::Decimal, text, output)?,
        ScalarType::Single => {
            if let Some(bits) = text.strip_prefix("bits:") {
                if bits.len() != 8 {
                    return Err(invalid());
                }
                Value::Single(Single::from_bits(
                    u32::from_str_radix(bits, 16).map_err(|_| invalid())?,
                ))
            } else {
                convert(Conversion::Single, text, output)?
            }
        }
        ScalarType::Double => {
            if let Some(bits) = text.strip_prefix("bits:") {
                if bits.len() != 16 {
                    return Err(invalid());
                }
                Value::Double(Double::from_bits(
                    u64::from_str_radix(bits, 16).map_err(|_| invalid())?,
                ))
            } else {
                convert(Conversion::Double, text, output)?
            }
        }
        ScalarType::Text => Value::Text(output.text(text)?),
        ScalarType::Binary => {
            if !text.len().is_multiple_of(2) {
                return Err(invalid());
            }
            let mut bytes = output.binary(text.len() / 2)?;
            for pair in text.as_bytes().as_chunks::<2>().0 {
                output.work(1)?;
                let pair = std::str::from_utf8(pair).map_err(|_| invalid())?;
                bytes.push(u8::from_str_radix(pair, 16).map_err(|_| invalid())?);
            }
            Value::Binary(bytes)
        }
        ScalarType::Guid => Value::Guid(Guid::from_bytes(
            uuid::Uuid::parse_str(text)
                .map_err(|_| invalid())?
                .into_bytes(),
        )),
        ScalarType::DateTime => {
            let value = if let Some(bits) = text.strip_prefix("bits:") {
                if bits.len() != 16 {
                    return Err(invalid());
                }
                LegacyDateTime::from_bits(u64::from_str_radix(bits, 16).map_err(|_| invalid())?)?
            } else {
                LegacyDateTime::from_calendar(
                    text.parse::<jiff::civil::DateTime>()
                        .map_err(|_| invalid())?,
                )?
            };
            Value::DateTime(value)
        }
        ScalarType::DateTimeExtended => {
            let civil = text
                .parse::<jiff::civil::DateTime>()
                .map_err(|_| invalid())?;
            if civil.subsec_nanosecond() % 100 != 0 {
                return Err(invalid());
            }
            Value::DateTimeExtended(ExtendedDateTime::new(
                civil.year().try_into().map_err(|_| invalid())?,
                civil.month().try_into().map_err(|_| invalid())?,
                civil.day().try_into().map_err(|_| invalid())?,
                civil.hour().try_into().map_err(|_| invalid())?,
                civil.minute().try_into().map_err(|_| invalid())?,
                civil.second().try_into().map_err(|_| invalid())?,
                (civil.subsec_nanosecond() / 100)
                    .try_into()
                    .map_err(|_| invalid())?,
            )?)
        }
    })
}

struct Buffer<'a, 'b> {
    output: &'a mut Presentation<'b>,
    text: String,
    failure: Option<Error>,
}

impl TextOutput for Buffer<'_, '_> {
    fn step(&mut self) -> Result<(), EvalError> {
        self.output.work(1).map_err(|error| {
            self.failure = Some(error);
            EvalError::Cancelled
        })
    }

    fn append(&mut self, fragment: &str) -> Result<(), EvalError> {
        self.output
            .append_text(&mut self.text, fragment)
            .map_err(|error| {
                self.failure = Some(error);
                EvalError::Cancelled
            })
    }
}

impl NumericBuffer for Buffer<'_, '_> {
    fn text(&self) -> &str {
        &self.text
    }
}

fn convert(
    conversion: Conversion,
    text: &str,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    let mut buffer = Buffer {
        output,
        text: String::new(),
        failure: None,
    };
    let result = conversion.apply_text(text, NumberFormat::Invariant, &mut buffer);
    if let Some(error) = buffer.failure {
        return Err(error);
    }
    Ok(result?)
}

/// Query bounds preserve entered decimal precision rather than storage rounding.
pub(crate) fn exact_numeric(text: &str, output: &mut Presentation<'_>) -> Result<Value, Error> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Error::ScalarInputLimit);
    }
    output.work(1 + text.len() as u64)?;
    let mut buffer = Buffer {
        output,
        text: String::new(),
        failure: None,
    };
    let result = decimal_exact(text, NumberFormat::Invariant, &mut buffer);
    if let Some(error) = buffer.failure {
        return Err(error);
    }
    Ok(result?)
}
