use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, math::random::RndState},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

#[test]
fn recognition_distinguishes_expression_states_from_explicit_code_conversion() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for function in [InspectionFunction::IsNumeric, InspectionFunction::IsDate] {
        for value in [
            ExpressionValue::Empty,
            ExpressionValue::Error(0),
            ExpressionValue::Error(65535),
            ExpressionValue::Error(u32::MAX),
            ExpressionValue::Value(Value::Null),
        ] {
            assert_eq!(
                evaluator.apply_expression_inspection(function, &value),
                Ok(Value::Boolean(false))
            );
        }
        assert_eq!(
            evaluator.apply_expression_inspection(function, &ExpressionValue::Missing),
            Err(EvalError::NonScalarExpression)
        );
        assert_eq!(
            evaluator.apply_expression_inspection(
                function,
                &ExpressionValue::Value(Value::Text("1".into()))
            ),
            Err(if matches!(function, InspectionFunction::IsNumeric) {
                EvalError::NumberFormatRequired
            } else {
                EvalError::DateFormatRequired
            })
        );
    }
    assert_eq!(
        evaluator.apply_expression_inspection(
            InspectionFunction::IsNumeric,
            &ExpressionValue::Value(Value::Int32(1))
        ),
        Ok(Value::Boolean(true))
    );
    assert_eq!(
        evaluator
            .with_number_format(NumberFormat::Invariant)
            .apply_expression_inspection(
                InspectionFunction::IsNumeric,
                &ExpressionValue::Value(Value::Text("1".into()))
            ),
        Ok(Value::Boolean(true))
    );
}

#[test]
fn state_recognition_preserves_cancellation_and_work_limits() {
    for function in [InspectionFunction::IsNumeric, InspectionFunction::IsDate] {
        for value in [
            ExpressionValue::Empty,
            ExpressionValue::Error(1),
            ExpressionValue::Missing,
        ] {
            let mut budget = ResourceBudget::new(0, 0, 0);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .apply_expression_inspection(function, &value),
                Err(EvalError::Limit(_))
            ));
            let mut budget = ResourceBudget::new(0, 0, 1000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| true)
                    .apply_expression_inspection(function, &value),
                Err(EvalError::Cancelled)
            );
        }
    }
}

#[test]
fn recognition_sql_preserves_operational_failures_and_volatile_draws() {
    for (sql, expected) in [
        ("SELECT IsNumeric(CVErr(1))", Ok(Value::Boolean(false))),
        ("SELECT IsDate(CVErr(1))", Ok(Value::Boolean(false))),
        ("SELECT IsNumeric(CLng(CVErr(1)))", Ok(Value::Boolean(true))),
        (
            "SELECT IsDate(CDate(CLng(CVErr(1))))",
            Ok(Value::Boolean(true)),
        ),
        ("SELECT IsNumeric(1/0)", Err(EvalError::DivisionByZero)),
        (
            "SELECT IsDate(IIf(True,CVErr(1),1/0))",
            Err(EvalError::DivisionByZero),
        ),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required")
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            expected,
            "{sql}"
        );
    }
    let parsed = parse_select(
        "SELECT IsNumeric(IIf(True,CVErr(1),Rnd()))",
        Default::default(),
    )
    .unwrap();
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
        Ok(Value::Boolean(false))
    );
    assert_eq!(state.apply(None), expected.apply(None));
}
