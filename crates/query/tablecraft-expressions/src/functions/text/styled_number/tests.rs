use super::*;
use crate::functions::text::{TextResult, apply_with_context};
use tablecraft_core::values::{Currency, Decimal, Double, ExtendedDateTime, LegacyDateTime};
#[derive(Default)]
struct Buffer {
    text: String,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.text.push_str(text);
        Ok(())
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.text
    }
}
fn call(
    function: TextFunction,
    arguments: &[Value],
    format: NumberFormat,
) -> Result<String, EvalError> {
    let TextResult::StyledNumber(recipe) = apply_with_context(
        function,
        arguments,
        TextContext {
            number_format: Some(format),
            ..Default::default()
        },
        || Ok(()),
    )?
    else {
        panic!()
    };
    let display = recipe.resolve(&mut Buffer::default())?;
    let mut output = Buffer::default();
    display.write(&mut output)?;
    Ok(output.text)
}
#[test]
fn named_defaults_and_explicit_controls_preserve_fixed_values() {
    let amount = Value::Currency(Currency::from_scaled(-12345678));
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            std::slice::from_ref(&amount),
            NumberFormat::EnUs
        )
        .unwrap(),
        "-1,234.57"
    );
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            std::slice::from_ref(&amount),
            NumberFormat::DeDe
        )
        .unwrap(),
        "-1.234,57"
    );
    assert_eq!(
        call(
            TextFunction::FormatCurrency,
            std::slice::from_ref(&amount),
            NumberFormat::EnUs
        )
        .unwrap(),
        "($1,234.57)"
    );
    assert_eq!(
        call(TextFunction::FormatCurrency, &[amount], NumberFormat::DeDe).unwrap(),
        "-1.234,57 €"
    );
    assert_eq!(
        call(
            TextFunction::FormatPercent,
            &[Value::Decimal(Decimal::new(12345, 6).unwrap())],
            NumberFormat::Invariant
        )
        .unwrap(),
        "1.23%"
    );
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            &[Value::Decimal(Decimal::new(5, 3).unwrap()), Value::Int16(2)],
            NumberFormat::Invariant
        )
        .unwrap(),
        "0.01"
    );
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            &[Value::Double(Double::from_float(0.005)), Value::Int16(2)],
            NumberFormat::Invariant
        )
        .unwrap(),
        "0.01"
    );
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            &[
                Value::Decimal(Decimal::new(5, 1).unwrap()),
                Value::Int16(2),
                Value::Boolean(false)
            ],
            NumberFormat::EnUs
        )
        .unwrap(),
        ".50"
    );
}
#[test]
fn percentage_display_and_wide_ieee_values_do_not_narrow_to_stored_carriers() {
    let maximum = Value::Decimal(Decimal::new(Decimal::MAX_COEFFICIENT, 0).unwrap());
    assert_eq!(
        call(
            TextFunction::FormatPercent,
            &[maximum, Value::Int16(0)],
            NumberFormat::Invariant
        )
        .unwrap(),
        "7922816251426433759354395033500%"
    );
    let large = call(
        TextFunction::FormatNumber,
        &[Value::Double(Double::from_float(f64::MAX)), Value::Int16(0)],
        NumberFormat::Invariant,
    )
    .unwrap();
    assert_eq!(large.len(), 309);
    assert!(large.starts_with("179769313486232"));
    assert!(large.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            &[Value::Double(Double::from_float(f64::from_bits(1)))],
            NumberFormat::Invariant
        )
        .unwrap(),
        "0.00"
    );
    assert_eq!(
        call(
            TextFunction::FormatNumber,
            &[Value::Int16(1), Value::Int16(255)],
            NumberFormat::Invariant
        )
        .unwrap(),
        format!("1.{}", "0".repeat(255))
    );
}

#[test]
fn options_null_nonfinite_profiles_and_private_scratch_are_explicit() {
    for (arguments, format, expected) in [
        (
            vec![Value::Null],
            NumberFormat::EnUs,
            EvalError::InvalidNullConversion,
        ),
        (
            vec![Value::Int16(1), Value::Null],
            NumberFormat::EnUs,
            EvalError::InvalidNullConversion,
        ),
        (
            vec![Value::Int16(1), Value::Int16(256)],
            NumberFormat::EnUs,
            EvalError::Domain,
        ),
        (
            vec![Value::Int16(1), Value::Int16(-2)],
            NumberFormat::EnUs,
            EvalError::Domain,
        ),
        (
            vec![Value::Int16(1), Value::Int16(2), Value::Int16(1)],
            NumberFormat::EnUs,
            EvalError::Domain,
        ),
        (
            vec![Value::Double(Double::from_float(f64::NAN))],
            NumberFormat::EnUs,
            EvalError::NonFinite,
        ),
        (
            vec![
                Value::Int16(1),
                Value::Int16(2),
                Value::Int16(-2),
                Value::Int16(-2),
                Value::Boolean(true),
            ],
            NumberFormat::Invariant,
            EvalError::UnsupportedExpression,
        ),
    ] {
        assert_eq!(
            call(TextFunction::FormatNumber, &arguments, format),
            Err(expected)
        );
    }
    assert_eq!(
        call(
            TextFunction::FormatCurrency,
            &[Value::Int16(1)],
            NumberFormat::Invariant
        ),
        Err(EvalError::UnsupportedExpression)
    );
    let arguments = [Value::Text("($1,234.50)".into())];
    let recipe = prepare(
        TextFunction::FormatCurrency,
        &arguments,
        TextContext {
            number_format: Some(NumberFormat::EnUs),
            ..Default::default()
        },
    )
    .unwrap();
    let mut scratch = Buffer::default();
    let display = recipe.resolve(&mut scratch).unwrap();
    assert!(!scratch.text.is_empty());
    let mut output = Buffer::default();
    display.write(&mut output).unwrap();
    assert_eq!(output.text, "($1,234.50)");
    assert!(matches!(
        prepare(
            TextFunction::FormatNumber,
            &[Value::Int16(1)],
            TextContext::default()
        ),
        Err(EvalError::NumberFormatRequired)
    ));
}
#[test]
fn host_failures_propagate_during_normalization_and_output() {
    struct Failing {
        calls: usize,
        failure: EvalError,
    }
    impl TextOutput for Failing {
        fn step(&mut self) -> Result<(), EvalError> {
            self.calls = self.calls.checked_sub(1).ok_or(self.failure)?;
            Ok(())
        }
        fn append(&mut self, _: &str) -> Result<(), EvalError> {
            Err(self.failure)
        }
    }
    impl NumericBuffer for Failing {
        fn text(&self) -> &str {
            ""
        }
    }
    let arguments = [Value::Text("1.25".into())];
    let recipe = prepare(
        TextFunction::FormatNumber,
        &arguments,
        TextContext {
            number_format: Some(NumberFormat::Invariant),
            ..Default::default()
        },
    )
    .unwrap();
    let display = recipe.resolve(&mut Buffer::default()).unwrap();
    for failure in [
        EvalError::Cancelled,
        EvalError::Allocation,
        EvalError::TextLimit,
        EvalError::TypeMismatch,
        EvalError::Overflow,
    ] {
        assert_eq!(
            recipe.resolve(&mut Failing { calls: 10, failure }),
            Err(failure)
        );
        assert_eq!(
            display.write(&mut Failing { calls: 10, failure }),
            Err(failure)
        );
    }
}
proptest::proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn fixed_currency_rounding_matches_independent_integer_half_away(scaled in proptest::prelude::any::<i64>(), places in 0_u8..=4) {
        for (function, multiplier, suffix) in [(TextFunction::FormatNumber, 1_i128, ""), (TextFunction::FormatPercent, 100_i128, "%")] {
            let value = i128::from(scaled) * multiplier;
            let divisor = 10_i128.pow(u32::from(4 - places));
            let magnitude = value.unsigned_abs();
            let rounded = magnitude / divisor as u128 + u128::from((magnitude % divisor as u128) * 2 >= divisor as u128);
            let scale = 10_u128.pow(u32::from(places));
            let sign = if value < 0 && rounded != 0 { "-" } else { "" };
            let expected = if places == 0 { format!("{sign}{rounded}{suffix}") } else { format!("{sign}{}.{:0width$}{suffix}", rounded / scale, rounded % scale, width=places as usize) };
            proptest::prop_assert_eq!(call(function, &[Value::Currency(Currency::from_scaled(scaled)), Value::Int16(i16::from(places))], NumberFormat::Invariant).unwrap(), expected);
        }
    }
}

#[test]
fn legacy_date_sources_display_raw_serials_without_calendar_rounding() {
    for (serial, number, percent, currency) in [
        (-1.25, "-1.25", "-125.00%", "($1.25)"),
        (-0.25, "-0.25", "-25.00%", "($0.25)"),
        (-0.0, "0.00", "0.00%", "$0.00"),
        (0.25, "0.25", "25.00%", "$0.25"),
        (45658.5, "45,658.50", "4,565,850.00%", "$45,658.50"),
        (
            -657434.75,
            "-657,434.75",
            "-65,743,475.00%",
            "($657,434.75)",
        ),
        (
            2958465.75,
            "2,958,465.75",
            "295,846,575.00%",
            "$2,958,465.75",
        ),
    ] {
        let date = LegacyDateTime::from_ole_days(serial).unwrap();
        let source = Value::DateTime(date);
        for (function, expected) in [
            (TextFunction::FormatNumber, number),
            (TextFunction::FormatPercent, percent),
            (TextFunction::FormatCurrency, currency),
        ] {
            assert_eq!(
                call(function, std::slice::from_ref(&source), NumberFormat::EnUs).unwrap(),
                expected
            );
        }
    }
    // A negative epoch alias retains its sign; civil conversion would erase it.
    let source = Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap());
    assert_eq!(
        call(TextFunction::FormatNumber, &[source], NumberFormat::DeDe).unwrap(),
        "-0,25"
    );
}

#[test]
fn extended_date_source_is_not_silently_narrowed_to_legacy_serial() {
    let source = Value::DateTimeExtended(ExtendedDateTime::new(2025, 1, 1, 0, 0, 0, 1).unwrap());
    for function in [
        TextFunction::FormatNumber,
        TextFunction::FormatPercent,
        TextFunction::FormatCurrency,
    ] {
        assert_eq!(
            call(function, std::slice::from_ref(&source), NumberFormat::EnUs),
            Err(EvalError::TypeMismatch)
        );
    }
}
