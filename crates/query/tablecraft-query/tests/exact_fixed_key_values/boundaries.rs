use super::*;

#[test]
fn all_decimal_scales_signs_zero_and_96_bit_boundaries_keep_first_payload() {
    let mut keys = Vec::new();
    for scale in (0..=28).rev() {
        // These independently authored equivalents fit below the 96-bit ceiling.
        let unit = 10_i128.pow(u32::from(scale));
        keys.extend([
            decimal(0, scale),
            decimal(unit, scale),
            decimal(-unit, scale),
            decimal(1, scale),
            decimal(-1, scale),
        ]);
    }
    let maximum = 79_228_162_514_264_337_593_543_950_335_i128;
    for coefficient in [maximum, maximum - 1, -maximum, -maximum + 1] {
        for scale in [0, 1, 27, 28] {
            keys.push(decimal(coefficient, scale));
        }
    }
    keys.extend([
        decimal(10, 0),
        decimal(100, 1),
        decimal(-1200, 3),
        decimal(-12, 1),
        Value::Null,
        Value::Null,
    ]);
    assert_scalar_population(ScalarType::Decimal, keys);
}

#[test]
fn currency_signed_minimum_and_adjacent_ten_thousandths_remain_exact() {
    let mut keys: Vec<_> = [
        i64::MIN,
        i64::MAX,
        -1,
        0,
        1,
        i64::MIN + 1,
        i64::MAX - 1,
        9_007_199_254_740_992,
        9_007_199_254_740_993,
        i64::MIN,
        0,
        i64::MAX,
    ]
    .into_iter()
    .map(currency)
    .collect();
    keys.extend([Value::Null, Value::Null]);
    assert_scalar_population(ScalarType::Currency, keys);
}

#[test]
fn decimal_currency_boolean_nullable_tuples_use_whole_key_rational_equality() {
    let rows = vec![
        vec![decimal(10, 1), currency(i64::MIN), Value::Boolean(true)],
        vec![decimal(100, 2), currency(i64::MIN), Value::Boolean(true)],
        vec![decimal(1, 0), currency(i64::MIN + 1), Value::Boolean(true)],
        vec![decimal(1000, 3), currency(i64::MIN), Value::Boolean(false)],
        vec![Value::Null, currency(0), Value::Null],
        vec![Value::Null, currency(0), Value::Null],
        vec![decimal(0, 28), currency(0), Value::Null],
        vec![decimal(0, 0), currency(0), Value::Null],
    ];
    let expected = groups(&rows);
    let source = source(
        &[
            ("D", ScalarType::Decimal),
            ("C", ScalarType::Currency),
            ("F", ScalarType::Boolean),
        ],
        rows,
    );
    for prepare_declared in [false, true] {
        for execute_declared in [false, true] {
            let actual = buffered(
                "SELECT D,C,F,Count(*) FROM (SELECT D,C,F FROM T) Q GROUP BY D,C,F",
                &source,
                prepare_declared,
                execute_declared,
            );
            let expected: Vec<_> = expected
                .iter()
                .map(|(key, count)| {
                    let mut row = key.clone();
                    row.push(Value::Int64(*count));
                    row
                })
                .collect();
            assert_eq!(actual, expected);
        }
    }
}
