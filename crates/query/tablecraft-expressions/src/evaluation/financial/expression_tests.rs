use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::math::random::RndState,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Double};
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn financial_empty_numeric_controls_use_zero_without_reinterpreting_currency_codes() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, numbers, empty_slot) in [
        (FinancialFunction::Pmt, vec![0, 10, 100], 0),
        (FinancialFunction::PV, vec![0, 10, 100], 0),
        (FinancialFunction::FV, vec![0, 10, 100], 0),
        (FinancialFunction::NPer, vec![0, -10, 100], 0),
        (FinancialFunction::IPmt, vec![0, 1, 10, 100], 0),
        (FinancialFunction::PPmt, vec![0, 1, 10, 100], 0),
        (FinancialFunction::SLN, vec![100, 0, 10], 1),
        (FinancialFunction::SYD, vec![100, 0, 10, 1], 1),
        (FinancialFunction::DDB, vec![100, 0, 10, 1], 1),
        (FinancialFunction::Rate, vec![10, -10, 100, 0, 0, 0], 5),
    ] {
        let numeric: Vec<Value> = numbers.into_iter().map(Value::Int16).collect();
        let mut arguments: Vec<ExpressionValue> = numeric
            .iter()
            .cloned()
            .map(ExpressionValue::Value)
            .collect();
        arguments[empty_slot] = ExpressionValue::Empty;
        assert_eq!(
            evaluator.apply_expression_financial(function, &arguments),
            Ok(function.apply(&numeric).unwrap())
        );
    }
    let mut euro = vec![
        ExpressionValue::Empty,
        ExpressionValue::Value(Value::Text("DEM".into())),
        ExpressionValue::Value(Value::Text("EUR".into())),
        ExpressionValue::Empty,
    ];
    assert_eq!(
        evaluator.apply_expression_financial(FinancialFunction::EuroConvert, &euro),
        Ok(Value::Double(Double::from_float(0.0)))
    );
    euro.push(ExpressionValue::Empty);
    assert_eq!(
        evaluator.apply_expression_financial(FinancialFunction::EuroConvert, &euro),
        Err(EvalError::Domain)
    );
    assert!(matches!(euro.pop(), Some(ExpressionValue::Empty)));
    euro[1] = ExpressionValue::Empty;
    assert_eq!(
        evaluator.apply_expression_financial(FinancialFunction::EuroConvert, &euro),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn financial_data_admission_retains_conversion_and_currency_validation_order() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (function, values, error) in [
        (
            FinancialFunction::SLN,
            vec![
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Value(Value::Int16(10)),
            ],
            EvalError::TypeMismatch,
        ),
        (
            FinancialFunction::SLN,
            vec![
                ExpressionValue::Value(Value::Null),
                ExpressionValue::Error(1),
                ExpressionValue::Value(Value::Int16(10)),
            ],
            EvalError::InvalidNullConversion,
        ),
        (
            FinancialFunction::SLN,
            vec![
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Missing,
                ExpressionValue::Value(Value::Int16(10)),
            ],
            EvalError::NonScalarExpression,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                ExpressionValue::Value(Value::Int16(1)),
                ExpressionValue::Value(Value::Text("USD".into())),
                ExpressionValue::Error(1),
            ],
            EvalError::Domain,
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                ExpressionValue::Error(u32::MAX),
                ExpressionValue::Value(Value::Text("USD".into())),
                ExpressionValue::Value(Value::Text("EUR".into())),
            ],
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(
            evaluator.apply_expression_financial(function, &values),
            Err(error)
        );
    }
}

#[test]
fn financial_sql_evaluates_later_operational_errors_before_admitting_error_data() {
    for sql in [
        "SELECT SLN(CVErr(1),1/0,10)",
        "SELECT Pmt(CVErr(1),10,1/0)",
        "SELECT EuroConvert(CVErr(1),'DEM','EUR',1/0)",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(EvalError::DivisionByZero)
        );
    }
}

#[test]
fn financial_expression_paths_share_entry_and_iteration_cancellation() {
    for (function, values) in [
        (
            FinancialFunction::Rate,
            vec![
                ExpressionValue::Value(Value::Int16(10)),
                ExpressionValue::Value(Value::Int16(-10)),
                ExpressionValue::Value(Value::Int16(100)),
                ExpressionValue::Empty,
            ],
        ),
        (
            FinancialFunction::SLN,
            vec![
                ExpressionValue::Value(Value::Int16(100)),
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Int16(10)),
            ],
        ),
        (
            FinancialFunction::EuroConvert,
            vec![
                ExpressionValue::Empty,
                ExpressionValue::Value(Value::Text("DEM".into())),
                ExpressionValue::Value(Value::Text("EUR".into())),
            ],
        ),
    ] {
        let checks = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_expression_financial(function, &values)
            .unwrap();
        for stop in 1..=checks.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| {
                        calls.set(calls.get() + 1);
                        calls.get() >= stop
                    })
                    .apply_expression_financial(function, &values),
                Err(EvalError::Cancelled)
            );
        }
    }
}

#[test]
fn financial_operands_execute_once_even_when_earlier_data_is_rejected() {
    let parsed = parse_select("SELECT SLN(CVErr(1),Rnd(),10)", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required")
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}
