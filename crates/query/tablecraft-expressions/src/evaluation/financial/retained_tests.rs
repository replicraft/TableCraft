use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExpressionString},
};

fn retained(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn cases() -> Vec<(FinancialFunction, Vec<&'static str>, f64)> {
    vec![
        (
            FinancialFunction::Pmt,
            vec!["0", "2", "100", "0", "0"],
            -50.0,
        ),
        (
            FinancialFunction::PV,
            vec!["0", "2", "-50", "0", "0"],
            100.0,
        ),
        (
            FinancialFunction::FV,
            vec!["0", "2", "-50", "0", "0"],
            100.0,
        ),
        (
            FinancialFunction::NPer,
            vec!["0", "-50", "100", "0", "0"],
            2.0,
        ),
        (
            FinancialFunction::IPmt,
            vec!["0", "1", "2", "100", "0", "0"],
            0.0,
        ),
        (
            FinancialFunction::PPmt,
            vec!["0", "1", "2", "100", "0", "0"],
            -50.0,
        ),
        (FinancialFunction::SLN, vec!["100", "0", "10"], 10.0),
        (FinancialFunction::SYD, vec!["100", "0", "4", "1"], 40.0),
        (
            FinancialFunction::DDB,
            vec!["1000", "0", "5", "1", "2"],
            400.0,
        ),
        (
            FinancialFunction::Rate,
            vec!["1", "-110", "100", "0", "0", "0.1"],
            0.1,
        ),
        (
            FinancialFunction::EuroConvert,
            vec!["1.95583", "DEM", "EUR", "1", "3.5"],
            1.0,
        ),
    ]
}

fn assert_number(result: Result<Value, EvalError>, expected: f64) {
    let Value::Double(actual) = result.unwrap() else {
        panic!("Double result required");
    };
    assert!(
        (actual.to_float() - expected).abs() <= 1e-10,
        "{} != {expected}",
        actual.to_float()
    );
}

#[test]
fn all_financial_calls_and_optional_slots_share_retained_and_materialized_semantics() {
    for (function, arguments, expected) in cases() {
        for count in function.required_arguments()..=arguments.len() {
            assert!(function.accepts_arity(count));
            for format in [NumberFormat::EnUs, NumberFormat::DeDe] {
                let arguments: Vec<_> = arguments[..count]
                    .iter()
                    .map(|text| {
                        if format == NumberFormat::DeDe {
                            text.replace('.', ",")
                        } else {
                            (*text).to_owned()
                        }
                    })
                    .collect();
                let materialized: Vec<_> = arguments.iter().cloned().map(Value::Text).collect();
                let retained: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
                let mut budget = ResourceBudget::new(65536, 0, 100000);
                let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(format);
                assert_number(evaluator.apply_financial(function, &materialized), expected);
                assert_number(
                    evaluator.apply_expression_financial(function, &retained),
                    expected,
                );
            }
        }
    }
}

fn malformed() -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]))
}

#[test]
fn financial_conversion_and_currency_admission_keep_progressive_failures() {
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (function, arguments, expected) in [
        (
            FinancialFunction::SLN,
            vec![malformed()],
            EvalError::UnsupportedExpression,
        ),
        (
            FinancialFunction::SLN,
            vec![
                ExpressionValue::Value(Value::Null),
                malformed(),
                retained("10"),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            FinancialFunction::SLN,
            vec![retained("1e999"), malformed(), retained("10")],
            EvalError::Overflow,
        ),
        (
            FinancialFunction::SLN,
            vec![retained("100"), ExpressionValue::Error(7), malformed()],
            EvalError::TypeMismatch,
        ),
        (
            FinancialFunction::Pmt,
            vec![retained("0"), retained("0"), malformed()],
            EvalError::UnrepresentableText,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                ExpressionValue::Value(Value::Null),
                malformed(),
                malformed(),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![retained("1"), retained("USD"), malformed()],
            EvalError::Domain,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![retained("1"), retained("EUR"), retained("USD"), malformed()],
            EvalError::Domain,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                retained("1"),
                retained("EUR"),
                retained("EUR"),
                ExpressionValue::Value(Value::Null),
                malformed(),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                retained("1"),
                retained("EUR"),
                retained("EUR"),
                retained("1"),
                retained("2"),
            ],
            EvalError::Domain,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![retained("1"), ExpressionValue::Empty, malformed()],
            EvalError::TypeMismatch,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![retained("1"), retained("EUR"), ExpressionValue::Missing],
            EvalError::NonScalarExpression,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_financial(function, &arguments),
            Err(expected),
            "{function:?}"
        );
    }

    let mut budget = ResourceBudget::new(65536, 0, 100000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.apply_expression_financial(
            FinancialFunction::EuroConvert,
            &[
                ExpressionValue::Value(Value::Int16(1)),
                retained("EUR"),
                retained("EUR"),
            ],
        ),
        Ok(Value::Double(Double::from_float(1.0)))
    );
    assert_eq!(
        evaluator.apply_expression_financial(
            FinancialFunction::SLN,
            &[retained("1"), malformed(), malformed()]
        ),
        Err(EvalError::NumberFormatRequired)
    );
    assert_eq!(
        evaluator.apply_expression_financial(
            FinancialFunction::SLN,
            &[malformed(), retained("0"), retained("10")]
        ),
        Err(EvalError::UnrepresentableText)
    );
}

#[test]
fn every_financial_call_shares_publication_conversion_and_solver_limits() {
    for (function, arguments, _) in cases() {
        let arguments: Vec<_> = arguments.iter().map(|text| retained(text)).collect();
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| {
                calls.set(calls.get() + 1);
                false
            })
            .apply_expression_financial(function, &arguments)
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
                    .apply_expression_financial(function, &arguments),
                Err(EvalError::Cancelled),
                "{function:?} cancellation {stop}"
            );
        }
        for (bytes, steps) in [(0, 100000), (65536, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::EnUs)
                    .apply_expression_financial(function, &arguments),
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
            .apply_expression_financial(function, &arguments),
            Err(EvalError::TextLimit)
        );
    }
}
