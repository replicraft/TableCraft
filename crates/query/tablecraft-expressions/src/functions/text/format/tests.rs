use super::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime};

#[derive(Default)]
struct Buffer(String);

impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.0.push_str(text);
        Ok(())
    }
}

impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.0
    }
}

pub(super) fn render(
    value: Value,
    mask: &str,
    number_format: Option<NumberFormat>,
) -> Result<String, EvalError> {
    let arguments = [value, Value::Text(mask.into())];
    let TextResult::Format(recipe) = prepare(
        &arguments,
        TextContext {
            number_format,
            ..Default::default()
        },
    )?
    else {
        panic!("expected custom Format recipe");
    };
    let display = recipe.resolve(&mut Buffer::default())?;
    let mut output = Buffer::default();
    display.write(&mut output)?;
    Ok(output.0)
}

#[test]
fn fixed_custom_masks_preserve_exact_rounding_padding_grouping_and_literals() {
    for (value, mask, expected) in [
        (Value::Int32(1234), "#,##0.00", "1,234.00"),
        (Value::Int32(12), "00000", "00012"),
        (Value::Int32(123456), "00", "123456"),
        (Value::Int32(0), "#", ""),
        (Value::Int32(0), "0", "0"),
        (Value::Int32(0), ".00", ".00"),
        (
            Value::Currency(Currency::from_scaled(123450)),
            "0.00##",
            "12.345",
        ),
        (
            Value::Decimal(Decimal::new(2345, 3).unwrap()),
            "0.00",
            "2.35",
        ),
        (Value::Double(Double::from_float(-2.5)), "0", "-3"),
        (
            Value::Currency(Currency::from_scaled(1250)),
            "0.00%",
            "12.50%",
        ),
        (Value::Int32(-12), "$0.00", "-$12.00"),
        (Value::Double(Double::from_float(0.125)), r"0.00\%", "0.13%"),
        (Value::Int32(7), r#""000"0"#, "0007"),
        (Value::Int32(7), r#""🦀; "0"#, "🦀; 7"),
    ] {
        assert_eq!(
            render(value, mask, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{mask}"
        );
    }
    assert_eq!(
        render(
            Value::Int32(12),
            "\"total; \"0\\;",
            Some(NumberFormat::EnUs)
        ),
        Ok("total; 12;".into())
    );
    assert_eq!(
        render(Value::Int32(1234), "#,##0.00", Some(NumberFormat::DeDe)),
        Ok("1.234,00".into())
    );
}

#[test]
fn sections_keep_negative_zero_and_null_routing_separate_from_quoted_semicolons() {
    let mask = "0.00;(0.00);\"zero\";\"missing\"";
    for (value, expected) in [
        (Value::Int32(12), "12.00"),
        (Value::Int32(-12), "(12.00)"),
        (Value::Int32(0), "zero"),
        (Value::Null, "missing"),
    ] {
        assert_eq!(
            render(value, mask, Some(NumberFormat::EnUs)),
            Ok(expected.into())
        );
    }
    assert_eq!(
        render(Value::Int32(-12), "0;;\"zero\"", Some(NumberFormat::EnUs)),
        Ok("-12".into())
    );
    assert_eq!(render(Value::Null, "0.00", None), Ok(String::new()));
    assert_eq!(
        render(Value::Null, "0;0;0;\"literal;semicolon\"", None),
        Ok("literal;semicolon".into())
    );
}

#[test]
fn named_numeric_and_boolean_formats_resolve_under_explicit_profiles() {
    for (value, name, expected) in [
        (Value::Int32(12), "Fixed", "12.00"),
        (Value::Int32(1234), "Standard", "1,234.00"),
        (
            Value::Currency(Currency::from_scaled(1250)),
            "Percent",
            "12.50%",
        ),
        (Value::Int32(-1234), "Currency", "($1,234.00)"),
        (Value::Int32(0), "Yes/No", "No"),
        (Value::Boolean(true), "True/False", "True"),
        (Value::Int32(-1), "On/Off", "On"),
        (Value::Text("1,234.5".into()), "0.00", "1234.50"),
    ] {
        assert_eq!(
            render(value, name, Some(NumberFormat::EnUs)),
            Ok(expected.into()),
            "{name}"
        );
    }
    assert_eq!(
        render(Value::Int32(-12), "Currency", Some(NumberFormat::DeDe)),
        Ok("-12,00 €".into())
    );
}

#[test]
fn malformed_and_unsupported_masks_do_not_silently_become_literal_output() {
    for mask in [
        "0E+",
        "0E00",
        "0.00,,",
        "0.0.0",
        "0%%",
        "[Red]0",
        "yyyyy-mm-dd",
        "@",
    ] {
        assert!(
            render(Value::Int32(1), mask, Some(NumberFormat::EnUs)).is_err(),
            "{mask}"
        );
    }
    for mask in ["0;0;0;0;0", "\"unfinished", "0\\"] {
        assert!(matches!(
            render(Value::Int32(1), mask, Some(NumberFormat::EnUs)),
            Err(EvalError::Domain)
        ));
    }
    assert_eq!(
        render(Value::Int32(1), "0.00", None),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        render(
            Value::Double(Double::from_float(f64::NAN)),
            "0",
            Some(NumberFormat::EnUs)
        ),
        Err(EvalError::NonFinite)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn integer_padding_and_sections_match_independent_decimal_spelling(
        value in -1_000_000i32..1_000_000,
        width in 1usize..20,
    ) {
        let mask = "0".repeat(width);
        let expected = if value < 0 {
            format!("-{:0width$}", value.unsigned_abs())
        } else {
            format!("{value:0width$}")
        };
        prop_assert_eq!(render(Value::Int32(value), &mask, Some(NumberFormat::Invariant)).unwrap(), expected);
    }
}

#[test]
fn format_mask_truncation_is_bounded_and_never_splits_a_surrogate_pair() {
    let mask = "0".repeat(257) + "[unsupported suffix]";
    let expected = "0".repeat(256) + "1";
    assert_eq!(
        render(Value::Int32(1), &mask, Some(NumberFormat::Invariant)),
        Ok(expected)
    );
    let mask = "0".to_owned() + &" ".repeat(255) + "🦀";
    assert_eq!(
        render(Value::Int32(1), &mask, Some(NumberFormat::Invariant)),
        Err(EvalError::UnrepresentableText)
    );
}

#[test]
fn numeric_date_masks_use_the_exact_stored_ole_serial_without_calendar_conversion() {
    for (serial, mask, expected) in [
        (45658.5, "0.00", "45658.50"),
        (-1.25, "0.00", "-1.25"),
        (-0.25, "0.00", "-0.25"),
        (0.25, "0.00", "0.25"),
        (0.0, "0.00;(0.00);\"zero\"", "zero"),
        (-1.25, "0.00;(0.00);\"zero\"", "(1.25)"),
        (45658.5, "0.00E+00", "4.57E+04"),
        (0.25, "0.0%", "25.0%"),
    ] {
        let date = LegacyDateTime::from_ole_days(serial).unwrap();
        let bits = date.to_bits();
        assert_eq!(
            render(Value::DateTime(date), mask, Some(NumberFormat::EnUs)),
            Ok(expected.into())
        );
        assert_eq!(date.to_bits(), bits);
    }
    let date = Value::DateTime(LegacyDateTime::from_ole_days(45658.5).unwrap());
    assert_eq!(
        render(date.clone(), "0.00", Some(NumberFormat::DeDe)),
        Ok("45658,50".into())
    );
    assert_eq!(
        render(date, "0.00", None),
        Err(EvalError::NumberFormatRequired)
    );
}
