use super::*;
use crate::evaluation::NoBindings;
use crate::{
    evaluation::PreparedNumericArithmetic,
    functions::{
        conversions::text::NumberFormat,
        text::byte_string::{ByteString, ExpressionByteOperand},
    },
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::{Double, ExpressionValue, Guid};
use tablecraft_sql::parse_expression;
use tablecraft_sql::select::BinaryOperator as B;

#[test]
fn raw_strings_allow_null_numeric_arithmetic_without_unicode_publication() {
    for operator in ["-", "*", "/", "\\", "Mod", "^"] {
        for raw in ["ChrW(55296)", "ChrB(65)", "LeftB('😀',2)"] {
            for sql in [
                format!("{raw} {operator} NULL"),
                format!("NULL {operator} {raw}"),
            ] {
                let expression = parse_expression(&sql, Default::default()).unwrap();
                let result = Evaluator::new(
                    &NoBindings,
                    &mut ResourceBudget::new(65536, 0, 10000),
                    Default::default(),
                )
                .evaluate(&expression);
                assert_eq!(result, Ok(Value::Null), "{sql}");
            }
        }
    }
}

#[test]
fn raw_numeric_sources_keep_state_errors_and_operand_family_admission() {
    for format in [None, Some(NumberFormat::EnUs)] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        if let Some(format) = format {
            evaluator = evaluator.with_number_format(format);
        }
        let result = evaluator.prepare_expression_numeric_arithmetic(
            B::Divide,
            ExpressionByteOperand::Bytes(ByteString::from_owned(vec![48, 0]).unwrap()),
            ExpressionByteOperand::Scalar(ExpressionValue::Empty),
        );
        let expected = if format.is_some() {
            EvalError::DivisionByZero
        } else {
            EvalError::NumberFormatRequired
        };
        assert!(matches!(result, Err(error) if error == expected));
    }

    for (value, expected) in [
        (ExpressionValue::Error(1), EvalError::TypeMismatch),
        (ExpressionValue::Missing, EvalError::NonScalarExpression),
        (
            ExpressionValue::Value(Value::Binary(vec![0])),
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Value(Value::Guid(Guid::from_bytes([0; 16]))),
            EvalError::TypeMismatch,
        ),
        (
            ExpressionValue::Value(Value::Double(Double::from_float(f64::INFINITY))),
            EvalError::NonFinite,
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let raw = ByteString::from_owned(vec![0, 216]).unwrap();
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .prepare_expression_numeric_arithmetic(
                B::Multiply,
                ExpressionByteOperand::Bytes(raw),
                ExpressionByteOperand::Scalar(value),
            );
        assert!(matches!(result, Err(error) if error == expected));
    }

    for (sql, expected) in [
        ("(ChrW(55296)-NULL)*(1/0)", EvalError::DivisionByZero),
        ("ChrW(65536)-NULL", EvalError::Domain),
        ("ChrW(55296)*1", EvalError::UnrepresentableText),
        ("LeftB('12',4)/2", EvalError::NumberFormatRequired),
    ] {
        let expression = parse_expression(sql, Default::default()).unwrap();
        let result = Evaluator::new(
            &NoBindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .evaluate(&expression);
        assert_eq!(result, Err(expected), "{sql}");
    }
}

#[test]
fn raw_numeric_arithmetic_budgets_publication_and_cancels_observed_work() {
    let raw = ByteString::from_owned(vec![0, 216]).unwrap();
    let mut budget = ResourceBudget::new(0, 0, 10000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .prepare_expression_numeric_arithmetic(
            B::Divide,
            ExpressionByteOperand::Bytes(raw),
            ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Null)),
        )
        .unwrap();
    assert!(matches!(
        result,
        PreparedNumericArithmetic::Result(Value::Null)
    ));

    let expression = parse_expression("LeftB('12',4)/CInt(2)", Default::default()).unwrap();
    let observed = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&|| {
            observed.set(observed.get() + 1);
            false
        })
        .evaluate(&expression)
        .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                })
                .evaluate(&expression),
            Err(EvalError::Cancelled),
            "checkpoint {stop}"
        );
    }
}
