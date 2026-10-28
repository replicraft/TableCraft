use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::byte_string::ByteString},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, ExpressionString, LegacyDateTime, Single},
};

fn scalar(value: Value) -> ExpressionByteOperand {
    ExpressionByteOperand::Scalar(ExpressionValue::Value(value))
}

fn raw(text: &str) -> ExpressionByteOperand {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionByteOperand::try_from(ExpressionValue::StringBytes(
        ExpressionString::from_owned(bytes).select(2..end).unwrap(),
    ))
    .unwrap()
}

fn run(
    evaluator: &mut Evaluator<'_>,
    left: ExpressionByteOperand,
    right: ExpressionByteOperand,
    left_declaration: Option<OperandDeclaration>,
    right_declaration: Option<OperandDeclaration>,
) -> Result<Option<Value>, EvalError> {
    match evaluator.prepare_declared_expression_addition(
        left,
        right,
        left_declaration,
        right_declaration,
    )? {
        Some(PreparedAddition::StringResult(value)) => {
            evaluator.publish_byte_operand(value).map(Some)
        }
        Some(PreparedAddition::ScalarOperands { left, right }) => evaluator
            .apply_binary(BinaryOperator::Add, &left, &right)
            .map(Some),
        None => Ok(None),
    }
}

#[test]
fn complete_mixed_plus_category_matrix_selects_error_numeric_or_string_in_both_orders() {
    let categories = [
        None,
        Some(OperandDeclaration::Scalar),
        Some(OperandDeclaration::Variant),
    ];
    for string_category in categories {
        for numeric_category in categories {
            for reverse in [false, true] {
                for retained in [false, true] {
                    let text = if retained {
                        raw("2")
                    } else {
                        scalar(Value::Text("2".into()))
                    };
                    let number = scalar(Value::Int16(3));
                    let (left, right, declarations) = if reverse {
                        (number, text, (numeric_category, string_category))
                    } else {
                        (text, number, (string_category, numeric_category))
                    };
                    let expected = match (string_category, numeric_category) {
                        (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Scalar)) => {
                            Err(EvalError::TypeMismatch)
                        }
                        (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Variant)) => {
                            Ok(Some(Value::Text(if reverse { "32" } else { "23" }.into())))
                        }
                        (Some(OperandDeclaration::Variant), _) => {
                            Ok(Some(Value::Double(Double::from_float(5.0))))
                        }
                        _ => Ok(None),
                    };
                    let mut budget = ResourceBudget::new(65536, 0, 10000);
                    let mut evaluator =
                        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                            .with_number_format(NumberFormat::EnUs);
                    assert_eq!(
                        run(&mut evaluator, left, right, declarations.0, declarations.1),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn declared_plus_retains_exact_numeric_carriers_and_scalar_formatting_policies() {
    for (number, formatted, sum) in [
        (
            Value::Boolean(true),
            "True",
            Value::Double(Double::from_float(1.0)),
        ),
        (Value::Byte(2), "2", Value::Double(Double::from_float(4.0))),
        (Value::Int16(2), "2", Value::Double(Double::from_float(4.0))),
        (Value::Int32(2), "2", Value::Double(Double::from_float(4.0))),
        (
            Value::Int64(9223372036854775807),
            "9223372036854775807",
            Value::Double(Double::from_float(9223372036854775808.0)),
        ),
        (
            Value::Single(Single::from_float(2.0)),
            "2",
            Value::Double(Double::from_float(4.0)),
        ),
        (
            Value::Double(Double::from_float(2.0)),
            "2",
            Value::Double(Double::from_float(4.0)),
        ),
        (
            Value::Currency(Currency::from_scaled(20000)),
            "2",
            Value::Currency(Currency::from_scaled(40000)),
        ),
        (
            Value::Decimal(Decimal::new(2, 0).unwrap()),
            "2",
            Value::Decimal(Decimal::new(4, 0).unwrap()),
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs);
        assert_eq!(
            run(
                &mut evaluator,
                raw("2"),
                scalar(number.clone()),
                Some(OperandDeclaration::Scalar),
                Some(OperandDeclaration::Variant)
            ),
            Ok(Some(Value::Text(format!("2{formatted}"))))
        );
        assert_eq!(
            run(
                &mut evaluator,
                raw("2"),
                scalar(number),
                Some(OperandDeclaration::Variant),
                Some(OperandDeclaration::Scalar)
            ),
            Ok(Some(sum))
        );
    }

    for (number, expected) in [
        (
            Value::Currency(Currency::from_scaled(9223372036854775807)),
            "n=922337203685477.5807",
        ),
        (
            Value::Decimal(Decimal::new(79228162514264337593543950335, 0).unwrap()),
            "n=79228162514264337593543950335",
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs);
        assert_eq!(
            run(
                &mut evaluator,
                raw("n="),
                scalar(number),
                Some(OperandDeclaration::Scalar),
                Some(OperandDeclaration::Variant),
            ),
            Ok(Some(Value::Text(expected.into())))
        );
    }

    let date = Value::DateTime(LegacyDateTime::from_ole_days(2.0).unwrap());
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        run(
            &mut evaluator,
            raw("date="),
            scalar(date),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Variant)
        ),
        Err(EvalError::DateFormatRequired)
    );
    assert_eq!(
        run(
            &mut evaluator,
            raw("2"),
            scalar(Value::Int16(3)),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Variant)
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        run(
            &mut evaluator,
            raw("2"),
            scalar(Value::Int16(3)),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Scalar)
        ),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        run(
            &mut evaluator,
            raw("2"),
            scalar(Value::Int16(3)),
            Some(OperandDeclaration::Variant),
            None
        ),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn strings_states_and_malformed_ranges_keep_branch_specific_admission() {
    let categories = [
        None,
        Some(OperandDeclaration::Scalar),
        Some(OperandDeclaration::Variant),
    ];
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for left_category in categories {
        for right_category in categories {
            assert_eq!(
                run(
                    &mut evaluator,
                    raw("a"),
                    raw("b"),
                    left_category,
                    right_category
                ),
                Ok(Some(Value::Text("ab".into())))
            );
            for reverse in [false, true] {
                for state in [
                    ExpressionValue::Empty,
                    ExpressionValue::Value(Value::Null),
                    ExpressionValue::Error(7),
                    ExpressionValue::Missing,
                ] {
                    let expected = match state {
                        ExpressionValue::Empty => Ok(Some(Value::Text("2".into()))),
                        ExpressionValue::Value(Value::Null) => Ok(Some(Value::Null)),
                        ExpressionValue::Error(_) => Err(EvalError::TypeMismatch),
                        _ => Err(EvalError::NonScalarExpression),
                    };
                    let (left, right) = if reverse {
                        (ExpressionByteOperand::Scalar(state), raw("2"))
                    } else {
                        (raw("2"), ExpressionByteOperand::Scalar(state))
                    };
                    assert_eq!(
                        run(&mut evaluator, left, right, left_category, right_category),
                        expected
                    );
                }
            }
        }
    }
    for (string, numeric, expected) in [
        (
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Scalar),
            Some(EvalError::TypeMismatch),
        ),
        (
            Some(OperandDeclaration::Variant),
            Some(OperandDeclaration::Scalar),
            Some(EvalError::UnrepresentableText),
        ),
        (None, Some(OperandDeclaration::Variant), None),
    ] {
        let bad = ExpressionByteOperand::Bytes(ByteString::from_owned(vec![0, 216]).unwrap());
        let result = run(
            &mut evaluator,
            bad,
            scalar(Value::Int16(1)),
            string,
            numeric,
        );
        assert_eq!(result, expected.map_or(Ok(None), Err));
    }
    let bad = ExpressionByteOperand::Bytes(ByteString::from_owned(vec![0, 216]).unwrap());
    let Some(PreparedAddition::StringResult(ByteOperand::Bytes(value))) = evaluator
        .prepare_declared_expression_addition(
            bad,
            scalar(Value::Int16(1)),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Variant),
        )
        .unwrap()
    else {
        panic!("raw concatenated result required");
    };
    assert_eq!(value.as_bytes(), [0, 216, b'1', 0]);
    let odd = ExpressionByteOperand::Bytes(ByteString::from_owned(vec![1]).unwrap());
    let Some(PreparedAddition::StringResult(ByteOperand::Bytes(value))) = evaluator
        .prepare_declared_expression_addition(
            odd,
            scalar(Value::Int16(1)),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Variant),
        )
        .unwrap()
    else {
        panic!("raw concatenated result required");
    };
    assert_eq!(value.as_bytes(), [1, b'1', 0]);
    assert_eq!(
        evaluator.publish_byte_operand(ByteOperand::Bytes(value)),
        Err(EvalError::UnrepresentableText)
    );
    let bad = ExpressionByteOperand::Bytes(ByteString::from_owned(vec![0, 216]).unwrap());
    assert!(matches!(
        evaluator.prepare_declared_expression_addition(
            bad,
            scalar(Value::Double(Double::from_float(f64::INFINITY))),
            None,
            None
        ),
        Err(EvalError::NonFinite)
    ));
}

#[test]
fn declared_mixed_plus_shares_formatting_copying_and_publication_limits() {
    let calls = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&cancelled);
    run(
        &mut evaluator,
        raw("id="),
        scalar(Value::Int64(9223372036854775807)),
        Some(OperandDeclaration::Scalar),
        Some(OperandDeclaration::Variant),
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled);
        assert_eq!(
            run(
                &mut evaluator,
                raw("id="),
                scalar(Value::Int64(9223372036854775807)),
                Some(OperandDeclaration::Scalar),
                Some(OperandDeclaration::Variant)
            ),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps) in [(0, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs);
        assert!(matches!(
            run(
                &mut evaluator,
                raw("id="),
                scalar(Value::Int16(1)),
                Some(OperandDeclaration::Scalar),
                Some(OperandDeclaration::Variant)
            ),
            Err(EvalError::Limit(_))
        ));
    }
}

#[test]
fn declared_string_numeric_join_enforces_the_final_text_ceiling() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 1,
            ..Default::default()
        },
    )
    .with_number_format(NumberFormat::EnUs);
    assert_eq!(
        run(
            &mut evaluator,
            raw("id="),
            scalar(Value::Int16(1)),
            Some(OperandDeclaration::Scalar),
            Some(OperandDeclaration::Variant),
        ),
        Err(EvalError::TextLimit)
    );
}
