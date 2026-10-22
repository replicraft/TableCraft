use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionState, ValueError},
};
use tablecraft_sql::{
    parse_select,
    select::{Expression, Projection},
};

fn expression(sql: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {sql}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expression required");
    };
    expression
}

#[test]
fn explicit_error_and_empty_conversion_remain_distinct_from_null_and_failures() {
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    for (sql, expected) in [
        ("CStr(CVErr(0))", "Error 0"),
        ("CStr(CVErr(2001))", "Error 2001"),
        ("CStr(CVErr(65535))", "Error 65535"),
        ("CStr(CVar(CVErr(2001)))", "Error 2001"),
        ("CStr(IIf(True,CVErr(2001),1))", "Error 2001"),
        ("CStr(Switch(True,CVErr(2001)))", "Error 2001"),
        ("CStr(Choose(1,CVErr(2001)))", "Error 2001"),
        ("CStr(Nz(Null))", ""),
        ("CStr(CVar(Nz(Null)))", ""),
        ("CStr(CVErr(2001)) & '!'", "Error 2001!"),
    ] {
        assert_eq!(
            evaluator.evaluate(&expression(sql)),
            Ok(Value::Text(expected.into())),
            "{sql}"
        );
    }
    for (sql, error) in [
        ("CStr(Null)", EvalError::InvalidNullConversion),
        ("CStr(1/0)", EvalError::DivisionByZero),
        ("CStr(IIf(True,CVErr(1),1/0))", EvalError::DivisionByZero),
        ("CStr(CVErr(-1))", EvalError::Domain),
        ("CVErr(1) & ''", EvalError::NonScalarExpression),
        ("CStr(1)", EvalError::NumberFormatRequired),
        ("CStr(1,1/0)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluator.evaluate(&expression(sql)), Err(error), "{sql}");
    }
}

#[test]
fn opaque_error_and_missing_values_require_explicit_semantics_and_never_gain_storage() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    for code in [65536, 0x80004005, u32::MAX] {
        assert_eq!(
            evaluator.apply_expression_cstr(&ExpressionValue::Error(code)),
            Err(EvalError::UnsupportedExpression)
        );
    }
    assert_eq!(
        evaluator.apply_expression_cstr(&ExpressionValue::Missing),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        Value::try_from(ExpressionValue::Error(1)),
        Err(ValueError::ExpressionNotStorable(ExpressionState::Error))
    );
    assert_eq!(
        Value::try_from(ExpressionValue::Empty),
        Err(ValueError::ExpressionNotStorable(ExpressionState::Empty))
    );
}

#[test]
fn error_spelling_ignores_numeric_separator_and_charges_exact_emitted_bytes() {
    for format in [
        NumberFormat::Invariant,
        NumberFormat::EnUs,
        NumberFormat::DeDe,
    ] {
        let mut budget = ResourceBudget::new(11, 0, 100);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(format)
            .apply_expression_cstr(&ExpressionValue::Error(65535));
        assert_eq!(result, Ok(Value::Text("Error 65535".into())));
        assert_eq!(budget.remaining(Resource::Bytes), 0);
    }
    let mut budget = ResourceBudget::new(10, 0, 100);
    assert!(
        matches!(Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_expression_cstr(&ExpressionValue::Error(65535)), Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
    );
    assert_eq!(
        budget.remaining(Resource::Bytes),
        4,
        "failed writes retain the prefix charge"
    );
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 6,
                ..Default::default()
            }
        )
        .apply_expression_cstr(&ExpressionValue::Error(1)),
        Err(EvalError::TextLimit)
    );
    let mut budget = ResourceBudget::new(100, 0, 0);
    assert!(
        matches!(Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_expression_cstr(&ExpressionValue::Error(1)), Err(EvalError::Limit(error)) if error.resource==Resource::Steps)
    );
}

#[test]
fn every_observed_error_conversion_checkpoint_can_cancel_including_empty_conversion() {
    let calls = Cell::new(0);
    let run = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&cancelled)
            .apply_expression_cstr(&ExpressionValue::Error(65535))
    };
    assert!(run(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        assert_eq!(run(stop), Err(EvalError::Cancelled), "checkpoint {stop}");
    }
    let mut budget = ResourceBudget::new(100, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&|| true)
            .apply_expression_cstr(&ExpressionValue::Empty),
        Err(EvalError::Cancelled)
    );
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4552524f52535452),
        ..Default::default()
    })]
    #[test]
    fn every_generated_standard_code_retains_its_exact_digits(code in proptest::prelude::any::<u16>()) {
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_cstr(&ExpressionValue::Error(u32::from(code)));
        proptest::prop_assert_eq!(result, Ok(Value::Text(format!("Error {code}"))));
    }
}
