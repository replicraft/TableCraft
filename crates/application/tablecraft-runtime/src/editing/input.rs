//! Exact field-directed input conversions, without SQL interpolation.
use super::{EditError, EditInput};
use crate::scalar_input::MAX_INPUT_BYTES;
use crate::{Error, display::Presentation};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

fn invalid() -> Error {
    EditError::InvalidInput.into()
}

fn parse(
    kind: ScalarType,
    input: &EditInput,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    crate::scalar_input::parse(kind, &input.text, input.is_null, output).map_err(
        |error| match error {
            Error::InvalidScalarInput => invalid(),
            Error::ScalarInputLimit => EditError::InputLimit.into(),
            error => error,
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single};

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
