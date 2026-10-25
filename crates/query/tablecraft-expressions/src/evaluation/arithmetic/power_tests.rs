use super::*;
use crate::{evaluation::NoBindings, functions::conversions::text::NumberFormat};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Double};
use tablecraft_sql::parse_expression;

#[test]
fn contextual_power_preserves_conversion_errors_before_the_domain_boundary() {
    for (sql, format, expected) in [
        ("'0'^'-1'", None, EvalError::NumberFormatRequired),
        (
            "'bad'^'-1'",
            Some(NumberFormat::Invariant),
            EvalError::TypeMismatch,
        ),
        (
            "0^ChrW(55296)",
            Some(NumberFormat::Invariant),
            EvalError::UnrepresentableText,
        ),
        ("(0.5^1075)^(-1)", None, EvalError::Domain),
        ("CDec(0)^CDate(-1)", None, EvalError::Domain),
    ] {
        let expression = parse_expression(sql, Default::default()).unwrap();
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        if let Some(format) = format {
            evaluator = evaluator.with_number_format(format);
        }
        assert_eq!(evaluator.evaluate(&expression), Err(expected), "{sql}");
    }
}

#[test]
fn power_conversion_work_cancellation_and_resources_do_not_become_domain_errors() {
    let expression = parse_expression("LeftB('0',2)^LeftB('-1',4)", Default::default()).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .with_cancellation(&count)
            .evaluate(&expression),
        Err(EvalError::Domain)
    );

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::Invariant)
                .with_cancellation(&cancelled)
                .evaluate(&expression),
            Err(EvalError::Cancelled),
            "checkpoint {stop}"
        );
    }

    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn contextual_power_keeps_exact_input_coercion_and_binary_result_signs() {
    for (sql, expected) in [
        ("(-1)^CDec('9007199254740993')", 1.0_f64),
        ("(-CDbl(0))^3", -0.0),
        ("(-0.5)^1075", -0.0),
    ] {
        let expression = parse_expression(sql, Default::default()).unwrap();
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .evaluate(&expression)
            .unwrap();
        assert_eq!(result, Value::Double(Double::from_float(expected)));
    }
}
