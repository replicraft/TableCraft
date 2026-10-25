use super::*;
use crate::{
    evaluation::{Bindings, EvalLimits, NoBindings},
    functions::{conditional::ConditionalFunction, conversions::text::NumberFormat},
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::ExpressionString};
use tablecraft_sql::{parse_expression, select::FieldReference};

fn raw(text: &str) -> ExpressionValue {
    let mut bytes = vec![0, 216];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let end = bytes.len();
    bytes.push(255);
    ExpressionValue::StringBytes(ExpressionString::from_owned(bytes).select(2..end).unwrap())
}

fn invalid() -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216]))
}

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let syntax = parse_expression(sql, Default::default()).unwrap();
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65536, 0, 10000),
        Default::default(),
    )
    .with_number_format(NumberFormat::Invariant)
    .evaluate(&syntax)
}

#[test]
fn all_three_control_consumers_share_materialized_and_active_string_conversion() {
    for format in [NumberFormat::Invariant, NumberFormat::DeDe] {
        for text in ["0", "1", "2.5"] {
            let text = if format == NumberFormat::DeDe {
                text.replace('.', ",")
            } else {
                text.to_owned()
            };
            for retained in [false, true] {
                let value = if retained {
                    raw(&text)
                } else {
                    ExpressionValue::Value(Value::Text(text.clone()))
                };
                let mut budget = ResourceBudget::new(65536, 0, 10000);
                let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_number_format(format);
                assert_eq!(
                    evaluator.apply_iif(&value, 11, 22),
                    Ok(if text == "0" { 22 } else { 11 })
                );

                let mut switch = ConditionalFunction::Switch.start_payload(4).unwrap();
                evaluator
                    .push_selection_control(&mut switch, value.clone())
                    .unwrap();
                evaluator.push_selection_payload(&mut switch, 11).unwrap();
                evaluator
                    .push_selection_control(
                        &mut switch,
                        ExpressionValue::Value(Value::Boolean(true)),
                    )
                    .unwrap();
                evaluator.push_selection_payload(&mut switch, 22).unwrap();
                assert_eq!(
                    switch.finish_payload(),
                    Ok(Some(if text == "0" { 22 } else { 11 }))
                );

                let mut choose = ConditionalFunction::Choose.start_payload(3).unwrap();
                evaluator
                    .push_selection_control(&mut choose, value)
                    .unwrap();
                evaluator.push_selection_payload(&mut choose, 11).unwrap();
                evaluator.push_selection_payload(&mut choose, 22).unwrap();
                let expected = match text.as_str() {
                    "0" => None,
                    "1" => Some(11),
                    _ => Some(22),
                };
                assert_eq!(choose.finish_payload(), Ok(expected));
            }
        }
    }
}

#[test]
fn malformed_control_conversion_waits_for_eager_branch_expression_errors() {
    for control in ["ChrW(55296)", "ChrB(49)", "'bad'"] {
        for sql in [
            format!("IIf({control},1/0,0)"),
            format!("Switch({control},1/0,True,0)"),
            format!("Choose({control},1/0,0)"),
        ] {
            assert_eq!(evaluate(&sql), Err(EvalError::DivisionByZero), "{sql}");
        }
    }
    for sql in [
        "IIf(ChrW(55296),1,2)",
        "Switch(ChrW(55296),1,True,2)",
        "Choose(ChrW(55296),1,2)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnrepresentableText), "{sql}");
    }
}

#[test]
fn deferred_unicode_errors_and_ignored_operational_failures_remain_latched() {
    for function in [ConditionalFunction::Switch, ConditionalFunction::Choose] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        let mut selection = function.start_payload(2).unwrap();
        assert_eq!(
            evaluator.push_selection_control(&mut selection, invalid()),
            Ok(())
        );
        evaluator.push_selection_payload(&mut selection, 7).unwrap();
        assert_eq!(
            selection.finish_payload(),
            Err(EvalError::UnrepresentableText)
        );

        let mut budget = ResourceBudget::new(0, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        let mut selection = function.start_payload(2).unwrap();
        assert!(matches!(
            evaluator.push_selection_control(&mut selection, raw("1")),
            Err(EvalError::Limit(_))
        ));
        let _ = evaluator.push_selection_payload(&mut selection, 7);
        assert!(matches!(
            selection.finish_payload(),
            Err(EvalError::Limit(_))
        ));
    }
}

struct Calls {
    count: Cell<usize>,
    cancelled: Cell<bool>,
    value: Value,
}

impl Calls {
    fn new() -> Self {
        Self {
            count: Cell::new(0),
            cancelled: Cell::new(false),
            value: Value::Int16(1),
        }
    }
}

impl Bindings for Calls {
    fn resolve(&self, _: &FieldReference) -> Result<&Value, EvalError> {
        assert!(
            !self.cancelled.get(),
            "No callback may follow observed cancellation"
        );
        self.count.set(self.count.get() + 1);
        Ok(&self.value)
    }
}

#[test]
fn each_eager_branch_resolves_once_but_invalid_constructors_stop_immediately() {
    for sql in [
        "IIf(ChrW(55296),A,B)",
        "Switch(ChrW(55296),A,True,B)",
        "Choose(ChrW(55296),A,B)",
    ] {
        let syntax = parse_expression(sql, Default::default()).unwrap();
        let bindings = Calls::new();
        let result = Evaluator::new(
            &bindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .evaluate(&syntax);
        assert_eq!(result, Err(EvalError::UnrepresentableText));
        assert_eq!(bindings.count.get(), 2, "{sql}");
    }
    for sql in [
        "IIf(ChrW(65536),A,B)",
        "Switch(ChrW(65536),A,True,B)",
        "Choose(ChrW(65536),A,B)",
    ] {
        let syntax = parse_expression(sql, Default::default()).unwrap();
        let bindings = Calls::new();
        let result = Evaluator::new(
            &bindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .evaluate(&syntax);
        assert_eq!(result, Err(EvalError::Domain));
        assert_eq!(bindings.count.get(), 0, "{sql}");
    }
}

#[test]
fn control_publication_keeps_text_limits_and_missing_policy_failures() {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.apply_iif(&raw("1"), 1, 2),
        Err(EvalError::NumberFormatRequired)
    );
    for function in [ConditionalFunction::Switch, ConditionalFunction::Choose] {
        let mut selection = function.start_payload(2).unwrap();
        evaluator
            .push_selection_control(&mut selection, raw("1"))
            .unwrap();
        evaluator.push_selection_payload(&mut selection, 7).unwrap();
        assert_eq!(
            selection.finish_payload(),
            Err(EvalError::NumberFormatRequired)
        );
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(
        &NoBindings,
        &mut budget,
        EvalLimits {
            max_text_bytes: 0,
            ..Default::default()
        },
    );
    assert_eq!(
        evaluator.apply_iif(&raw("1"), 1, 2),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn every_control_and_branch_checkpoint_can_cancel_without_later_callbacks() {
    for sql in [
        "IIf(ChrW(49),A,B)",
        "Switch(ChrW(49),A,True,B)",
        "Choose(ChrW(49),A,B)",
    ] {
        let syntax = parse_expression(sql, Default::default()).unwrap();
        let observed = Cell::new(0);
        let observe = || {
            observed.set(observed.get() + 1);
            false
        };
        let bindings = Calls::new();
        Evaluator::new(
            &bindings,
            &mut ResourceBudget::new(65536, 0, 10000),
            Default::default(),
        )
        .with_number_format(NumberFormat::Invariant)
        .with_cancellation(&observe)
        .evaluate(&syntax)
        .unwrap();
        assert_eq!(bindings.count.get(), 2);
        for stop in 1..=observed.get() {
            let bindings = Calls::new();
            let calls = Cell::new(0);
            let cancel = || {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    bindings.cancelled.set(true);
                }
                bindings.cancelled.get()
            };
            let result = Evaluator::new(
                &bindings,
                &mut ResourceBudget::new(65536, 0, 10000),
                Default::default(),
            )
            .with_number_format(NumberFormat::Invariant)
            .with_cancellation(&cancel)
            .evaluate(&syntax);
            assert_eq!(
                result,
                Err(EvalError::Cancelled),
                "{sql}; checkpoint {stop}"
            );
        }
    }
}
