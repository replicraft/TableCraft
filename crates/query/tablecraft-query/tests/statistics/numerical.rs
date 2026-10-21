use super::*;
use tablecraft_core::values::{Currency, Decimal, Single};

#[test]
fn finite_carriers_use_existing_double_conversion_without_text_or_date_coercion() {
    for (kind, values, expected) in [
        (ScalarType::Byte, vec![Value::Byte(1), Value::Byte(3)], 1.0),
        (
            ScalarType::Int16,
            vec![Value::Int16(-1), Value::Int16(1)],
            1.0,
        ),
        (
            ScalarType::Int64,
            vec![Value::Int64(1), Value::Int64(3)],
            1.0,
        ),
        (
            ScalarType::Boolean,
            vec![Value::Boolean(true), Value::Boolean(false)],
            0.25,
        ),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(10000)),
                Value::Currency(Currency::from_scaled(30000)),
            ],
            1.0,
        ),
        (
            ScalarType::Decimal,
            vec![
                Value::Decimal(Decimal::new(125, 2).unwrap()),
                Value::Decimal(Decimal::new(375, 2).unwrap()),
            ],
            1.5625,
        ),
        (
            ScalarType::Single,
            vec![
                Value::Single(Single::from_float(1.0)),
                Value::Single(Single::from_float(3.0)),
            ],
            1.0,
        ),
    ] {
        near(
            float(
                &run("SELECT VarP(X) FROM T", &source(kind, values))
                    .unwrap()
                    .rows()[0][0],
            ),
            expected,
        );
    }
    let data = source(
        ScalarType::Int64,
        vec![
            Value::Int64(9_007_199_254_740_992),
            Value::Int64(9_007_199_254_740_993),
        ],
    );
    assert_eq!(
        float(&run("SELECT VarP(X) FROM T", &data).unwrap().rows()[0][0]),
        0.0
    );
}
#[test]
fn shifting_scaling_preserve_large_offset_tiny_spreads_and_representable_deviation() {
    for (values, expected) in [
        (vec![1e16, 1e16 + 2.0, 1e16 + 4.0], (8.0_f64 / 3.0).sqrt()),
        (vec![1e-200, 3e-200], 1e-200),
        (vec![f64::MAX, -f64::MAX], f64::MAX),
        (vec![f64::MAX, f64::MAX], 0.0),
    ] {
        let data = source(
            ScalarType::Double,
            values
                .into_iter()
                .map(|v| Value::Double(Double::from_float(v)))
                .collect(),
        );
        near(
            float(&run("SELECT StDevP(X) FROM T", &data).unwrap().rows()[0][0]),
            expected,
        );
    }
    let data = source(
        ScalarType::Double,
        vec![
            Value::Double(Double::from_float(f64::MAX)),
            Value::Double(Double::from_float(-f64::MAX)),
        ],
    );
    assert_eq!(
        run("SELECT VarP(X) FROM T", &data).err(),
        Some(EvalError::Overflow.into())
    );
    assert_eq!(
        run("SELECT StDev(X) FROM T", &data).err(),
        Some(EvalError::Overflow.into())
    );
}
