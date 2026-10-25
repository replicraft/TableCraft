use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, ExpressionString},
};
use tablecraft_sql::parse_expression;

const OPERATORS: [B; 8] = [
    B::Add,
    B::Subtract,
    B::Multiply,
    B::Divide,
    B::IntegerDivide,
    B::Modulo,
    B::Power,
    B::Concatenate,
];

fn raw(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn scalar(value: Value) -> ExpressionValue {
    ExpressionValue::Value(value)
}

fn apply(operator: B, left: ExpressionValue, right: ExpressionValue) -> Result<Value, EvalError> {
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .with_number_format(NumberFormat::Invariant)
    .apply_expression_binary(operator, left, right)
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn all_eight_operators_admit_retained_active_strings_like_materialized_text() {
    for (operator, forward, reverse) in [
        (B::Add, double(6.0), double(6.0)),
        (B::Subtract, double(2.0), double(-2.0)),
        (B::Multiply, double(8.0), double(8.0)),
        (B::Divide, double(2.0), double(0.5)),
        (B::IntegerDivide, Value::Int32(2), Value::Int16(0)),
        (B::Modulo, Value::Int32(0), Value::Int16(2)),
        (B::Power, double(16.0), double(16.0)),
        (
            B::Concatenate,
            Value::Text("42".into()),
            Value::Text("24".into()),
        ),
    ] {
        for (reverse_order, expected) in [(false, forward), (true, reverse)] {
            let operands = |text| {
                if reverse_order {
                    (scalar(Value::Int16(2)), text)
                } else {
                    (text, scalar(Value::Int16(2)))
                }
            };
            let (left, right) = operands(scalar(Value::Text("4".into())));
            assert_eq!(apply(operator, left, right), Ok(expected.clone()));
            let (left, right) = operands(raw("4"));
            assert_eq!(
                apply(operator, left, right),
                Ok(expected),
                "{operator:?}; reverse {reverse_order}"
            );
        }
    }
}

#[test]
fn dispatcher_results_match_sql_walk_for_all_retained_operator_paths() {
    for (operator, spelling) in [
        (B::Add, "+"),
        (B::Subtract, "-"),
        (B::Multiply, "*"),
        (B::Divide, "/"),
        (B::IntegerDivide, "\\"),
        (B::Modulo, "Mod"),
        (B::Power, "^"),
        (B::Concatenate, "&"),
    ] {
        for reverse in [false, true] {
            let (sql, left, right) = if reverse {
                (
                    format!("CInt(2) {spelling} LeftB('4',2)"),
                    scalar(Value::Int16(2)),
                    raw("4"),
                )
            } else {
                (
                    format!("LeftB('4',2) {spelling} CInt(2)"),
                    raw("4"),
                    scalar(Value::Int16(2)),
                )
            };
            let expression = parse_expression(&sql, Default::default()).unwrap();
            let expected = Evaluator::new(
                &NoBindings,
                &mut ResourceBudget::new(65536, 0, 10000),
                Default::default(),
            )
            .with_number_format(NumberFormat::Invariant)
            .evaluate(&expression);
            assert_eq!(apply(operator, left, right), expected, "{sql}");
        }
    }
}

#[test]
fn retained_null_empty_and_error_states_keep_the_owning_operator_rules() {
    for operator in OPERATORS {
        for reverse in [false, true] {
            let (left, right) = if reverse {
                (scalar(Value::Null), raw("4"))
            } else {
                (raw("4"), scalar(Value::Null))
            };
            let expected = if operator == B::Concatenate {
                Value::Text("4".into())
            } else {
                Value::Null
            };
            assert_eq!(apply(operator, left, right), Ok(expected));

            for (peer, expected) in [
                (ExpressionValue::Error(1), EvalError::TypeMismatch),
                (ExpressionValue::Missing, EvalError::NonScalarExpression),
            ] {
                let malformed =
                    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]));
                let (left, right) = if reverse {
                    (peer, malformed)
                } else {
                    (malformed, peer)
                };
                assert_eq!(apply(operator, left, right), Err(expected));
            }
        }
    }

    for (operator, expected) in [
        (B::Add, Ok(Value::Text("4".into()))),
        (B::Subtract, Ok(double(4.0))),
        (B::Multiply, Ok(double(0.0))),
        (B::Divide, Err(EvalError::DivisionByZero)),
        (B::IntegerDivide, Err(EvalError::DivisionByZero)),
        (B::Modulo, Err(EvalError::DivisionByZero)),
        (B::Power, Ok(double(1.0))),
        (B::Concatenate, Ok(Value::Text("4".into()))),
    ] {
        assert_eq!(apply(operator, raw("4"), ExpressionValue::Empty), expected);
    }
}

#[test]
fn invalid_selected_ranges_fail_only_at_the_required_publication_boundary() {
    for bytes in [vec![0, 216], vec![65]] {
        for operator in OPERATORS {
            let invalid =
                || ExpressionValue::StringBytes(ExpressionString::from_owned(bytes.clone()));
            assert_eq!(
                apply(operator, invalid(), scalar(Value::Int16(2))),
                Err(EvalError::UnrepresentableText),
                "{operator:?}",
            );
            let expected = if operator == B::Concatenate {
                Err(EvalError::UnrepresentableText)
            } else {
                Ok(Value::Null)
            };
            assert_eq!(apply(operator, invalid(), scalar(Value::Null)), expected);
        }
    }
}

#[test]
fn exact_longlong_fixed_carriers_and_missing_number_policies_are_preserved() {
    assert_eq!(
        apply(
            B::IntegerDivide,
            raw("9007199254740993"),
            scalar(Value::Int64(1))
        ),
        Ok(Value::Int64(9007199254740993)),
    );
    assert_eq!(
        apply(
            B::Multiply,
            raw("1.25"),
            scalar(Value::Decimal(Decimal::new(2, 0).unwrap()))
        ),
        Ok(Value::Decimal(Decimal::new(250, 2).unwrap())),
    );
    assert_eq!(
        apply(
            B::Subtract,
            raw("1.25"),
            scalar(Value::Currency(Currency::from_scaled(10000)))
        ),
        Ok(Value::Currency(Currency::from_scaled(2500))),
    );
    for operator in OPERATORS {
        if operator == B::Concatenate {
            continue;
        }
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)));
        assert_eq!(result, Err(EvalError::NumberFormatRequired), "{operator:?}");
    }
}

#[test]
fn dispatcher_publication_observes_shared_limits_and_every_cancel_checkpoint() {
    for operator in OPERATORS {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 0,
                ..Default::default()
            },
        )
        .with_number_format(NumberFormat::Invariant)
        .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)));
        assert_eq!(result, Err(EvalError::TextLimit), "{operator:?}");

        let mut budget = ResourceBudget::new(65536, 0, 0);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)));
        assert!(matches!(result, Err(EvalError::Limit(_))), "{operator:?}");

        let mut budget = ResourceBudget::new(0, 0, 10000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)));
        assert!(matches!(result, Err(EvalError::Limit(_))), "{operator:?}");

        let observed = Cell::new(0);
        let observe = || {
            observed.set(observed.get() + 1);
            false
        };
        Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .with_number_format(NumberFormat::Invariant)
        .with_cancellation(&observe)
        .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)))
        .unwrap();
        for stop in 1..=observed.get() {
            let calls = Cell::new(0);
            let cancel = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::Invariant)
                .with_cancellation(&cancel)
                .apply_expression_binary(operator, raw("4"), scalar(Value::Int16(2)));
            assert_eq!(
                result,
                Err(EvalError::Cancelled),
                "{operator:?}; checkpoint {stop}"
            );
        }
    }
}
