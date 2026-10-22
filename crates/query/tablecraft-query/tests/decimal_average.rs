use tablecraft_core::values::{Decimal, Value};
use tablecraft_query::{
    ExecutionOptions, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn average(values: &[(i128, u8)]) -> Decimal {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("Amount".into(), ScalarType::Decimal, false).unwrap()],
            values
                .iter()
                .map(|(coefficient, scale)| {
                    vec![Value::Decimal(Decimal::new(*coefficient, *scale).unwrap())]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT Avg(Amount), DAvg('Amount','T') FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows()[0][0], result.rows()[0][1]);
    let Value::Decimal(value) = result.rows()[0][0] else {
        panic!("Decimal mean lost its carrier");
    };
    value
}

#[test]
fn decimal_means_survive_totals_larger_than_the_96_bit_carrier() {
    let maximum = 79228162514264337593543950335_i128;
    for (values, expected) in [
        (
            vec![(maximum, 0), (maximum, 0)],
            Decimal::new(maximum, 0).unwrap(),
        ),
        (
            vec![(-maximum, 0), (-maximum, 0)],
            Decimal::new(-maximum, 0).unwrap(),
        ),
        (
            vec![(maximum, 28), (maximum, 28)],
            Decimal::new(maximum, 28).unwrap(),
        ),
    ] {
        assert_eq!(average(&values), expected);
    }
}

#[test]
fn mixed_scales_keep_tiny_residuals_through_large_cancellation() {
    let maximum = 79228162514264337593543950335_i128;
    assert_eq!(
        average(&[(maximum, 0), (3, 28), (-maximum, 0)]),
        Decimal::new(1, 28).unwrap()
    );
}

#[test]
fn wide_decimal_means_round_halfway_coefficients_to_even() {
    let maximum = 79228162514264337593543950335_i128;
    assert_eq!(
        average(&[(maximum, 0), (maximum - 1, 0)]),
        Decimal::new(maximum - 1, 0).unwrap()
    );
    assert_eq!(
        average(&[(-maximum, 0), (-maximum + 1, 0)]),
        Decimal::new(-maximum + 1, 0).unwrap()
    );
}
