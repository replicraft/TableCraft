use super::*;
use crate::functions::financial::FinancialFunction;
use proptest::prelude::*;
use tablecraft_core::values::Decimal;

fn convert(amount: f64, source: &str, target: &str, options: &[Value]) -> Result<f64, EvalError> {
    let mut arguments = vec![
        Value::Double(Double::from_float(amount)),
        Value::Text(source.into()),
        Value::Text(target.into()),
    ];
    arguments.extend_from_slice(options);
    let Value::Double(value) = FinancialFunction::EuroConvert.apply(&arguments)? else {
        panic!("Double required");
    };
    Ok(value.to_float())
}

#[test]
fn official_rates_identity_target_rounding_and_full_precision_are_distinct() {
    for (source, rate, places) in [
        ("ATS", 13.7603, 2),
        ("BEF", 40.3399, 0),
        ("DEM", 1.95583, 2),
        ("ESP", 166.386, 0),
        ("FIM", 5.94573, 2),
        ("FRF", 6.55957, 2),
        ("IEP", 0.787564, 2),
        ("ITL", 1936.27, 0),
        ("LUF", 40.3399, 0),
        ("NLG", 2.20371, 2),
        ("PTE", 200.482, 0),
        ("EUR", 1.0, 2),
    ] {
        assert!(
            (convert(rate, source, "EUR", &[Value::Boolean(true)]).unwrap() - 1.0).abs() < 1e-14
        );
        assert_eq!(convert(1.23456789, source, source, &[]), Ok(1.23456789));
        let amount = convert(1.0, "EUR", source, &[]).unwrap();
        let expected = if source == "EUR" {
            1.0
        } else {
            (rate * 10_f64.powi(places)).round() / 10_f64.powi(places)
        };
        assert_eq!(amount, expected, "{source}");
    }
    assert_eq!(convert(1.2, "dem", "eur", &[]), Ok(0.61));
    assert_eq!(convert(-1.2, "DEM", "EUR", &[]), Ok(-0.61));
    assert_eq!(convert(1.47, "FRF", "DEM", &[]), Ok(0.44));
    assert_eq!(convert(0.1, "EUR", "PTE", &[]), Ok(20.0));
    assert_eq!(convert(1.005, "EUR", "EUR", &[]), Ok(1.005));
}

#[test]
fn triangulation_uses_significant_digits_only_between_national_currencies() {
    let value = convert(
        1000.0,
        "FRF",
        "DEM",
        &[Value::Boolean(true), Value::Int16(3)],
    )
    .unwrap();
    assert!((value - 152.0 * 1.95583).abs() < 1e-12);
    let unrounded = convert(1000.0, "FRF", "DEM", &[Value::Boolean(true)]).unwrap();
    assert!((unrounded - (1000.0 / 6.55957) * 1.95583).abs() < 1e-12);
    assert_ne!(value, unrounded);
    assert_eq!(
        convert(
            1000.0,
            "FRF",
            "DEM",
            &[Value::Boolean(true), Value::Int16(i16::MAX)]
        ),
        Ok(unrounded)
    );
    assert_eq!(
        convert(1.2, "DEM", "EUR", &[Value::Boolean(true), Value::Int16(3)]),
        convert(1.2, "DEM", "EUR", &[Value::Boolean(true)])
    );
    assert_eq!(significant(0.001234567, 3), Ok(0.00123));
    assert_eq!(significant(-999.5, 3), Ok(-1000.0));
}

#[test]
fn finite_extremes_invalid_parameters_and_underflow_are_explicit() {
    assert_eq!(convert(f64::MAX, "DEM", "DEM", &[]), Ok(f64::MAX));
    assert_eq!(
        convert(f64::MAX, "EUR", "ITL", &[]),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        convert(f64::from_bits(1), "ITL", "DEM", &[Value::Boolean(true)]),
        Err(EvalError::PrecisionLoss)
    );
    for (source, target) in [
        ("USD", "EUR"),
        ("EUR", "GBP"),
        ("DEM ", "EUR"),
        ("€", "EUR"),
        ("BGN", "EUR"),
    ] {
        assert_eq!(convert(1.0, source, target, &[]), Err(EvalError::Domain));
    }
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            convert(number, "EUR", "EUR", &[]),
            Err(EvalError::NonFinite)
        );
    }
    for argument in [Value::Int16(2), Value::Int16(0), Value::Int16(-1)] {
        assert_eq!(
            convert(1.0, "DEM", "EUR", &[Value::Boolean(true), argument]),
            Err(EvalError::Domain)
        );
    }
    assert_eq!(
        convert(1.0, "DEM", "EUR", &[Value::Null]),
        Err(EvalError::InvalidNullConversion)
    );
    let arguments = [
        Value::Decimal(Decimal::new(12, 1).unwrap()),
        Value::Text("DEM".into()),
        Value::Text("EUR".into()),
    ];
    assert_eq!(
        FinancialFunction::EuroConvert.apply(&arguments),
        Ok(Value::Double(Double::from_float(0.61)))
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn equal_currency_returns_the_original_double_without_rounding(value in -1e100_f64..1e100_f64) {
        prop_assert_eq!(convert(value, "BEF", "BEF", &[]).unwrap().to_bits(), value.to_bits());
    }
}
