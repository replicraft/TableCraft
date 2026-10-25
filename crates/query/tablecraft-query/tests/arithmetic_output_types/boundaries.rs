//! Exercise prepared type guarantees against independently chosen value envelopes.
use super::*;

fn samples() -> Vec<(ScalarType, Vec<Value>)> {
    vec![
        (
            ScalarType::Boolean,
            vec![Value::Boolean(false), Value::Boolean(true)],
        ),
        (
            ScalarType::Byte,
            [0, 1, 127, 128, 255].map(Value::Byte).to_vec(),
        ),
        (
            ScalarType::Int16,
            [i16::MIN, -1, 0, 1, i16::MAX].map(Value::Int16).to_vec(),
        ),
        (
            ScalarType::Int32,
            [i32::MIN, -32_769, -1, 0, 1, 32_768, i32::MAX]
                .map(Value::Int32)
                .to_vec(),
        ),
        (
            ScalarType::Int64,
            [i64::MIN, -2_147_483_649, -1, 0, 1, 2_147_483_648, i64::MAX]
                .map(Value::Int64)
                .to_vec(),
        ),
        (
            ScalarType::Single,
            [-f32::MAX, -1.5, -0.0, 0.0, f32::from_bits(1), 1.5, f32::MAX]
                .map(|value| Value::Single(Single::from_float(value)))
                .to_vec(),
        ),
        (
            ScalarType::Double,
            [-f64::MAX, -1.5, -0.0, 0.0, f64::from_bits(1), 1.5, f64::MAX]
                .map(|value| Value::Double(Double::from_float(value)))
                .to_vec(),
        ),
        (
            ScalarType::Currency,
            [i64::MIN, -10_001, -1, 0, 1, 10_001, i64::MAX]
                .map(|value| Value::Currency(Currency::from_scaled(value)))
                .to_vec(),
        ),
        (
            ScalarType::Decimal,
            [
                (-Decimal::MAX_COEFFICIENT, 0),
                (-15, 1),
                (-1, 28),
                (0, 28),
                (1, 28),
                (15, 1),
                (Decimal::MAX_COEFFICIENT, 0),
            ]
            .map(|(coefficient, scale)| Value::Decimal(Decimal::new(coefficient, scale).unwrap()))
            .to_vec(),
        ),
        (
            ScalarType::DateTime,
            [-657_434.75, -1.5, -0.25, -0.0, 0.0, 0.25, 1.5, 2_958_465.75]
                .map(|value| Value::DateTime(LegacyDateTime::from_ole_days(value).unwrap()))
                .to_vec(),
        ),
        (
            ScalarType::Text,
            [
                "",
                "bad",
                "-1.5",
                "0",
                "1.5",
                "9223372036854775807",
                "1e300",
            ]
            .map(|value| Value::Text(value.into()))
            .to_vec(),
        ),
    ]
}

fn runtime(left: (ScalarType, Value), right: (ScalarType, Value)) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("A".into(), left.0, true).unwrap(),
                Column::new("B".into(), right.0, true).unwrap(),
            ],
            vec![vec![left.1, right.1]],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn prepared_arithmetic_types_hold_at_carrier_boundaries_and_nulls() {
    let mut successful = 0;
    let mut refused = 0;

    for (left_kind, mut left_values) in samples() {
        left_values.push(Value::Null);
        for (right_kind, mut right_values) in samples() {
            right_values.push(Value::Null);
            let schema = NoScan {
                columns: vec![
                    Column::new("A".into(), left_kind, true).unwrap(),
                    Column::new("B".into(), right_kind, true).unwrap(),
                ],
            };

            for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
                let sql = format!("SELECT A {operator} B FROM T");
                let plan = prepare(&syntax(&sql), &schema).unwrap();
                let descriptor = plan.output_types().unwrap()[0];

                for left in &left_values {
                    for right in &right_values {
                        let source =
                            runtime((left_kind, left.clone()), (right_kind, right.clone()));
                        let result = plan.execute(
                            &source,
                            &[],
                            ExecutionOptions {
                                number_format: Some(NumberFormat::EnUs),
                                ..Default::default()
                            },
                        );
                        let result = match result {
                            Ok(result) => result,
                            Err(QueryError::Evaluation(
                                EvalError::Overflow
                                | EvalError::PrecisionLoss
                                | EvalError::NonFinite
                                | EvalError::DivisionByZero
                                | EvalError::Domain
                                | EvalError::TypeMismatch
                                | EvalError::UnsupportedNumericCombination,
                            )) => {
                                refused += 1;
                                continue;
                            }
                            Err(error) => panic!("{left:?} {operator} {right:?}: {error:?}"),
                        };

                        successful += 1;
                        assert_eq!(
                            result.output_types(),
                            [materialized_descriptor(
                                descriptor, operator, left_kind, right_kind
                            )]
                        );
                        let value = &result.rows()[0][0];
                        match descriptor {
                            OutputType::Scalar {
                                data_type,
                                nullable,
                            } => {
                                assert!(nullable || *value != Value::Null);
                                assert!(
                                    data_type.validate_value(value).is_ok(),
                                    "{left:?} {operator} {right:?}: {value:?} violates {descriptor:?}"
                                );
                            }
                            OutputType::Null => assert_eq!(*value, Value::Null),
                            OutputType::Dynamic => {}
                        }
                    }
                }
            }
        }
    }

    assert!(successful > 0);
    assert!(refused > 0);
    assert_eq!(successful + refused, 44_800);
    eprintln!("arithmetic metadata: {successful} successful values, {refused} checked refusals");
}

#[test]
fn prepared_unary_types_hold_at_carrier_boundaries_and_nulls() {
    let mut successful = 0;
    let mut refused = 0;

    for (kind, mut values) in samples() {
        values.push(Value::Null);
        let schema = NoScan {
            columns: vec![
                Column::new("A".into(), kind, true).unwrap(),
                Column::new("B".into(), ScalarType::Int32, true).unwrap(),
            ],
        };
        for operator in ["+", "-", "Not"] {
            let plan = prepare(&syntax(&format!("SELECT {operator} A FROM T")), &schema).unwrap();
            let descriptor = plan.output_types().unwrap()[0];

            for input in &values {
                let source = runtime((kind, input.clone()), (ScalarType::Int32, Value::Int32(0)));
                let result = plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        number_format: Some(NumberFormat::EnUs),
                        ..Default::default()
                    },
                );
                let result = match result {
                    Ok(result) => result,
                    Err(QueryError::Evaluation(
                        EvalError::Overflow
                        | EvalError::PrecisionLoss
                        | EvalError::NonFinite
                        | EvalError::TypeMismatch,
                    )) => {
                        refused += 1;
                        continue;
                    }
                    Err(error) => panic!("{operator} {input:?}: {error:?}"),
                };

                successful += 1;
                assert_eq!(result.output_types(), [descriptor]);
                let value = &result.rows()[0][0];
                match descriptor {
                    OutputType::Scalar {
                        data_type,
                        nullable,
                    } => {
                        assert!(nullable || *value != Value::Null);
                        assert!(
                            data_type.validate_value(value).is_ok(),
                            "{operator} {input:?}: {value:?} violates {descriptor:?}"
                        );
                    }
                    OutputType::Null => assert_eq!(*value, Value::Null),
                    OutputType::Dynamic => {}
                }
            }
        }
    }

    assert!(successful > 0);
    assert!(refused > 0);
    assert_eq!(successful + refused, 240);
    eprintln!("unary metadata: {successful} successful values, {refused} checked refusals");
}
