//! Exact field-directed input conversions, without SQL interpolation.
use super::{EditError, EditInput};
use crate::{Error, display::Presentation};
use tablecraft_core::values::{Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value};
use tablecraft_expressions::{
    EvalError,
    functions::{
        conversions::{Conversion, text::NumberFormat},
        text::output::{NumericBuffer, TextOutput},
    },
};
use tablecraft_schema::ScalarType;

const MAX_INPUT_BYTES: usize = 1024 * 1024;

pub(super) fn kind(kind: ScalarType) -> &'static str {
    match kind {
        ScalarType::Boolean => "Boolean: true or false",
        ScalarType::Byte => "Byte: 0 to 255",
        ScalarType::Int16 => "Signed 16-bit integer",
        ScalarType::Int32 => "Signed 32-bit integer",
        ScalarType::Int64 => "Signed 64-bit integer",
        ScalarType::Single => "Single: decimal number",
        ScalarType::Double => "Double: decimal number",
        ScalarType::Currency => "Currency: decimal amount (four decimal places)",
        ScalarType::Decimal => "Decimal number",
        ScalarType::Text => "Literal text",
        ScalarType::Binary => "Binary: hexadecimal byte pairs",
        ScalarType::Guid => "GUID: canonical identifier",
        ScalarType::DateTime => "Legacy DATE: ISO civil date-time (no time zone)",
        ScalarType::DateTimeExtended => {
            "Extended date-time: ISO civil date-time (100 ns precision)"
        }
    }
}

fn invalid() -> Error {
    EditError::InvalidInput.into()
}

fn parse(
    kind: ScalarType,
    input: &EditInput,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    if input.text.len() > MAX_INPUT_BYTES {
        return Err(EditError::InputLimit.into());
    }
    output.work(1 + input.text.len() as u64)?;
    if input.is_null {
        return Ok(Value::Null);
    }
    let text = input.text.as_str();
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

pub(super) fn initial(value: &Value, output: &mut Presentation<'_>) -> Result<EditInput, Error> {
    let text = match value {
        Value::Null => output.text("")?,
        Value::Single(value) => output.text(&value.to_float().to_string())?,
        Value::Double(value) => output.text(&value.to_float().to_string())?,
        Value::DateTime(value) => output.text(&value.calendar_rounded_to_nanos()?.to_string())?,
        Value::Binary(_) => {
            let mut text = output.value(value)?;
            output.work(text.len() as u64)?;
            text.drain(..2);
            text
        }
        _ => output.value(value)?,
    };
    Ok(EditInput {
        text,
        is_null: matches!(value, Value::Null),
    })
}

// Unchanged initial DATE/float text retains the exact original stored carrier.
// Rounded calendar display must not silently become replacement DATE bits.
pub(super) fn parse_original(
    kind: ScalarType,
    input: &EditInput,
    original: &Value,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    if input.text.len() > MAX_INPUT_BYTES {
        return Err(EditError::InputLimit.into());
    }
    if !input.is_null
        && matches!(
            original,
            Value::DateTime(_) | Value::Single(_) | Value::Double(_)
        )
    {
        let initial = initial(original, output)?;
        output.work(1 + input.text.len() as u64 + initial.text.len() as u64)?;
        if input.text == initial.text {
            return Ok(match original {
                Value::DateTime(value) => Value::DateTime(*value),
                Value::Single(value) => Value::Single(*value),
                Value::Double(value) => Value::Double(*value),
                _ => return Err(invalid()),
            });
        }
    }
    parse(kind, input, output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal};

    #[test]
    fn human_numeric_inputs_use_existing_conversions_and_integers_do_not_round() {
        let mut output = Presentation::new(&|| false);
        let amount = EditInput {
            text: "12.3456".into(),
            is_null: false,
        };
        assert_eq!(
            parse(ScalarType::Currency, &amount, &mut output).unwrap(),
            Value::Currency(Currency::from_scaled(123456))
        );
        assert_eq!(
            parse(ScalarType::Decimal, &amount, &mut output).unwrap(),
            Value::Decimal(Decimal::new(123456, 4).unwrap())
        );
        assert!(parse(ScalarType::Int32, &amount, &mut output).is_err());
        let text = EditInput {
            text: "NULL".into(),
            is_null: false,
        };
        assert_eq!(
            parse(ScalarType::Text, &text, &mut output).unwrap(),
            Value::Text("NULL".into())
        );
        let null = EditInput {
            text: "ignored draft text".into(),
            is_null: true,
        };
        assert_eq!(
            parse(ScalarType::Text, &null, &mut output).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn unchanged_human_date_and_float_initials_keep_original_exact_bits() {
        let mut output = Presentation::new(&|| false);
        for (kind, value) in [
            (
                ScalarType::DateTime,
                Value::DateTime(LegacyDateTime::from_bits(1).unwrap()),
            ),
            (
                ScalarType::DateTime,
                Value::DateTime(LegacyDateTime::from_bits(0x8000000000000000).unwrap()),
            ),
            (
                ScalarType::Single,
                Value::Single(Single::from_bits(0x80000000)),
            ),
            (ScalarType::Double, Value::Double(Double::from_bits(1))),
        ] {
            let initial = initial(&value, &mut output).unwrap();
            assert!(!initial.text.starts_with("bits:"));
            assert_eq!(
                parse_original(kind, &initial, &value, &mut output).unwrap(),
                value
            );
        }
        let civil = EditInput {
            text: "2026-11-04T12:34:56".into(),
            is_null: false,
        };
        let Value::DateTime(value) = parse(ScalarType::DateTime, &civil, &mut output).unwrap()
        else {
            panic!("expected DATE");
        };
        assert_eq!(value.calendar_rounded_to_nanos().unwrap().year(), 2026);
    }
}
