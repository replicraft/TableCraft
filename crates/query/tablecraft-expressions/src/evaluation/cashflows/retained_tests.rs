use super::*;
use crate::{evaluation::EvalLimits, functions::conversions::text::NumberFormat};
use std::cell::Cell;
use tablecraft_core::values::ExpressionString;

fn retained(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn cases() -> [(CashFlowFunction, Vec<&'static str>, f64); 3] {
    [
        (CashFlowFunction::NPV, vec!["0"], 21.0),
        (CashFlowFunction::IRR, vec!["0.1"], 0.1),
        (CashFlowFunction::MIRR, vec!["0", "0"], 0.1),
    ]
}

fn flows() -> [ExpressionValue; 3] {
    [-100, 0, 121].map(|value| ExpressionValue::Value(Value::Int16(value)))
}

fn malformed() -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]))
}

#[test]
fn cashflow_scalar_controls_keep_contextual_text_results_and_strict_array_admission() {
    for (function, arguments, expected) in cases() {
        for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
            let texts: Vec<_> = arguments
                .iter()
                .map(|text| {
                    if format == NumberFormat::DeDe {
                        text.replace('.', ",")
                    } else {
                        (*text).to_owned()
                    }
                })
                .collect();
            for retained_input in [false, true] {
                let controls: Vec<_> = texts
                    .iter()
                    .map(|text| {
                        if retained_input {
                            retained(text)
                        } else {
                            ExpressionValue::Value(Value::Text(text.clone()))
                        }
                    })
                    .collect();
                let mut budget = ResourceBudget::new(65536, 0, 100000);
                let Value::Double(actual) =
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .with_number_format(format)
                        .apply_expression_cashflows(function, &controls, &flows())
                        .unwrap()
                else {
                    panic!("Double result required");
                };
                assert!((actual.to_float() - expected).abs() <= 1e-10);
            }
        }
        for (element, expected) in [
            (
                ExpressionValue::Value(Value::Text("1".into())),
                EvalError::TypeMismatch,
            ),
            (retained("1"), EvalError::NonScalarExpression),
            (ExpressionValue::Empty, EvalError::TypeMismatch),
            (ExpressionValue::Error(7), EvalError::TypeMismatch),
        ] {
            let controls: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
            let mut budget = ResourceBudget::new(65536, 0, 100000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .apply_expression_cashflows(function, &controls, &[element]),
                Err(expected)
            );
        }
    }
}

#[test]
fn cashflow_scalar_publication_keeps_array_rate_and_guess_failure_order() {
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    let null = [ExpressionValue::Value(Value::Null)];
    assert_eq!(
        evaluator.apply_expression_cashflows(CashFlowFunction::NPV, &[malformed()], &null),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        evaluator.apply_expression_cashflows(CashFlowFunction::IRR, &[malformed()], &null),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        evaluator.apply_expression_cashflows(
            CashFlowFunction::MIRR,
            &[malformed(), malformed()],
            &null
        ),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        evaluator.apply_expression_cashflows(
            CashFlowFunction::MIRR,
            &[retained("bad"), malformed()],
            &flows()
        ),
        Err(EvalError::TypeMismatch)
    );
    for (function, arguments, _) in cases() {
        let mut arguments: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
        arguments[0] = malformed();
        assert_eq!(
            evaluator.apply_expression_cashflows(function, &arguments, &flows()),
            Err(EvalError::UnrepresentableText)
        );
    }
}

#[test]
fn retained_cashflow_controls_share_cumulative_work_bytes_text_and_cancellation() {
    for (function, arguments, _) in cases() {
        let controls: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| {
                calls.set(calls.get() + 1);
                false
            })
            .apply_expression_cashflows(function, &controls, &flows())
            .unwrap();
        for stop in 1..=calls.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 100000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_cashflows(function, &controls, &flows()),
                Err(EvalError::Cancelled)
            );
        }
        for (bytes, steps) in [(0, 100000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .apply_expression_cashflows(function, &controls, &flows()),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 0,
                    ..Default::default()
                }
            )
            .with_number_format(NumberFormat::EnUs)
            .apply_expression_cashflows(function, &controls, &flows()),
            Err(EvalError::TextLimit)
        );
    }
}
