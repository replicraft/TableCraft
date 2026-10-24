use super::*;
use crate::{
    evaluation::{Bindings, NoBindings},
    functions::{conditional::ConditionalFunction, conversions::text::NumberFormat},
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    parse_select,
    select::{FieldReference, Projection},
};

struct TestBindings {
    calls: Cell<usize>,
    first_check: Cell<usize>,
    checks: Cell<usize>,
    value: Value,
}

impl Bindings for TestBindings {
    fn resolve(&self, _: &FieldReference) -> Result<&Value, EvalError> {
        self.calls.set(self.calls.get() + 1);
        if self.first_check.get() == 0 {
            self.first_check.set(self.checks.get());
        }
        Ok(&self.value)
    }
}

fn evaluate(text: &str, profile: Option<NumberFormat>) -> Result<ExpressionValue, EvalError> {
    let parsed = parse_select(&format!("SELECT {text}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    if let Some(profile) = profile {
        evaluator = evaluator.with_number_format(profile);
    }
    evaluator.evaluate_expression_value(expression)
}

#[test]
fn iif_selects_lossless_strings_after_evaluating_both_branches() {
    for (sql, expected) in [
        ("LenB(IIf(True,ChrW(55296),ChrB(65)))", Value::Int32(2)),
        ("LenB(IIf(False,ChrW(55296),ChrB(65)))", Value::Int32(1)),
        ("IsNull(IIf(True,ChrW(55296),NULL))", Value::Boolean(false)),
        ("IIf(False,ChrW(55296),'ok')", Value::Text("ok".into())),
    ] {
        assert_eq!(
            evaluate(sql, None),
            Ok(ExpressionValue::Value(expected)),
            "{sql}"
        );
    }

    assert_eq!(
        evaluate("IIf(True,CVErr(1),ChrW(55296))", None),
        Ok(ExpressionValue::Error(1))
    );
}

#[test]
fn text_selectors_use_explicit_policies_without_converting_choice_payloads() {
    for (sql, profile, expected) in [
        (
            "IIf('True','chosen',0)",
            NumberFormat::EnUs,
            ExpressionValue::Value(Value::Text("chosen".into())),
        ),
        (
            "IIf('0,25',CVErr(2001),0)",
            NumberFormat::DeDe,
            ExpressionValue::Error(2001),
        ),
        (
            "Switch('False',1,'2,5','text')",
            NumberFormat::DeDe,
            ExpressionValue::Value(Value::Text("text".into())),
        ),
        (
            "Choose('2,5',1,CVErr(2001),3)",
            NumberFormat::DeDe,
            ExpressionValue::Error(2001),
        ),
        (
            "Switch('1','0,25')",
            NumberFormat::DeDe,
            ExpressionValue::Value(Value::Text("0,25".into())),
        ),
        (
            "Choose('1e30',1,2)",
            NumberFormat::Invariant,
            ExpressionValue::Value(Value::Null),
        ),
        (
            "Eval(\"IIf('1',CVErr(2001),0)\")",
            NumberFormat::Invariant,
            ExpressionValue::Error(2001),
        ),
    ] {
        assert_eq!(evaluate(sql, Some(profile)), Ok(expected), "{sql}");
    }
    for sql in ["IIf('1',1,0)", "Switch('1',1)", "Choose('1',1)"] {
        assert_eq!(
            evaluate(sql, None),
            Err(EvalError::NumberFormatRequired),
            "{sql}"
        );
    }
    for sql in ["IIf('',1,0)", "Switch('',1)", "Choose('',1)"] {
        assert_eq!(
            evaluate(sql, Some(NumberFormat::EnUs)),
            Err(EvalError::TypeMismatch)
        );
    }
    for sql in [
        "IIf(CVErr(1),1,0)",
        "Switch(True,1,CVErr(0),2)",
        "Choose(CVErr(1),1)",
    ] {
        assert_eq!(
            evaluate(sql, Some(NumberFormat::EnUs)),
            Err(EvalError::TypeMismatch),
            "{sql}"
        );
    }
    for sql in [
        "IIf('bad',1,1/0)",
        "Switch('bad',1,True,1/0)",
        "Choose('bad',1,1/0)",
    ] {
        for profile in [None, Some(NumberFormat::EnUs)] {
            assert_eq!(
                evaluate(sql, profile),
                Err(EvalError::DivisionByZero),
                "{sql}"
            );
        }
    }
}

#[test]
fn cancellation_during_conversion_stops_before_later_host_callbacks() {
    let parsed = parse_select("SELECT Switch('1',Later,True,Later)", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let bindings = TestBindings {
        calls: Cell::new(0),
        first_check: Cell::new(0),
        checks: Cell::new(0),
        value: Value::Int16(7),
    };
    let observed = || {
        bindings.checks.set(bindings.checks.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 100_000);
    Evaluator::new(&bindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&observed)
        .evaluate(expression)
        .unwrap();
    for stop in 1..=bindings.first_check.get() {
        bindings.calls.set(0);
        bindings.checks.set(0);
        let cancel = || {
            bindings.checks.set(bindings.checks.get() + 1);
            bindings.checks.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 100_000);
        assert_eq!(
            Evaluator::new(&bindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&cancel)
                .evaluate(expression),
            Err(EvalError::Cancelled)
        );
        assert_eq!(bindings.calls.get(), 0, "checkpoint {stop}");
    }
    bindings.calls.set(0);
    let mut budget = ResourceBudget::new(1, 0, 100_000);
    assert!(
        matches!(Evaluator::new(&bindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs).evaluate(expression),
        Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
    );
    assert_eq!(bindings.calls.get(), 0);
}

#[test]
fn contextual_selection_charges_text_before_fallible_normalization() {
    let mut selection = ConditionalFunction::Choose.start(2).unwrap();
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::Invariant)
            .push_selection_argument(
                &mut selection,
                ExpressionValue::Value(Value::Text("1".into()))
            ),
        Err(EvalError::Limit(_))
    ));
    selection.push(Value::Int16(7)).unwrap();
    assert!(matches!(selection.finish(), Err(EvalError::Limit(_))));
}
