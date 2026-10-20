use super::*;
use crate::EvalError;
use std::cell::RefCell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::{
    ParseLimits, parse_select,
    select::{Expression, FieldReference, Projection},
};
fn expression(sql: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {sql}"), ParseLimits::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expression");
    };
    expression
}
struct Ordered {
    values: [Value; 3],
    calls: RefCell<Vec<String>>,
}
impl Bindings for Ordered {
    fn resolve(&self, field: &FieldReference) -> Result<&Value, EvalError> {
        self.calls.borrow_mut().push(field.name.as_str().into());
        let index = ["condition", "yes", "no"]
            .iter()
            .position(|name| *name == field.name.as_str())
            .ok_or(EvalError::UnboundField)?;
        Ok(&self.values[index])
    }
}
#[test]
fn calls_evaluate_in_order_preserve_branch_types_and_do_not_hide_unselected_failures() {
    let bindings = Ordered {
        values: [Value::Boolean(true), Value::Int16(7), Value::Int32(9)],
        calls: RefCell::new(vec![]),
    };
    let mut budget = ResourceBudget::new(1024, 0, 100);
    let mut evaluator = Evaluator::new(&bindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.evaluate(&expression("IIf([condition],[yes],[no])")),
        Ok(Value::Int16(7))
    );
    assert_eq!(*bindings.calls.borrow(), ["condition", "yes", "no"]);
    for sql in ["IIf(True,1,1/0)", "IIf(False,1/0,1)", "IsNull(1/0)"] {
        assert_eq!(
            evaluator.evaluate(&expression(sql)),
            Err(EvalError::DivisionByZero)
        );
    }
    assert_eq!(
        evaluator.evaluate(&expression("IIf(Null,1,2)")),
        Ok(Value::Int64(2))
    );
}
#[test]
fn unselected_text_still_consumes_limits_and_cancelled_calls_never_visit_bindings() {
    let mut budget = ResourceBudget::new(3, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert!(
        matches!(evaluator.evaluate(&expression("IIf(True,'yes','unused')")),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
    );
    let bindings = Ordered {
        values: [Value::Boolean(true), Value::Null, Value::Null],
        calls: RefCell::new(vec![]),
    };
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(&bindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .evaluate(&expression("IIf([condition],[yes],[no])")),
        Err(EvalError::Cancelled)
    );
    assert!(bindings.calls.borrow().is_empty());
}
#[test]
fn call_shape_is_validated_before_evaluating_arguments() {
    let mut budget = ResourceBudget::new(0, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for sql in [
        "IIf(1/0,1)",
        "IIf(True,,0)",
        "IsNull(*)",
        "VBA.IIf(True,1,0)",
        "NeverRunMe(1/0)",
    ] {
        assert_eq!(
            evaluator.evaluate(&expression(sql)),
            Err(EvalError::UnsupportedExpression)
        );
    }
}

#[test]
fn nz_query_defaults_and_eager_fallbacks_share_budget_and_order() {
    let bindings = Ordered {
        values: [Value::Null, Value::Int16(7), Value::Int32(9)],
        calls: RefCell::new(vec![]),
    };
    let mut budget = ResourceBudget::new(1024, 0, 100);
    let mut evaluator = Evaluator::new(&bindings, &mut budget, EvalLimits::default());
    for (sql, expected) in [
        ("Nz(Null)", Value::Text(String::new())),
        ("Nz(Null,Null)", Value::Null),
        ("Nz(Null,0)", Value::Int64(0)),
        ("Nz('',42)", Value::Text(String::new())),
        ("Nz([yes],[no])", Value::Int16(7)),
    ] {
        assert_eq!(evaluator.evaluate(&expression(sql)), Ok(expected));
    }
    assert_eq!(*bindings.calls.borrow(), ["yes", "no"]);
    assert_eq!(
        evaluator.evaluate(&expression("Nz(1,1/0)")),
        Err(EvalError::DivisionByZero)
    );
    for sql in ["Nz(*)", "Nz(Null,)", "Nz(1,2,3)", "VBA.Nz(1)"] {
        assert_eq!(
            evaluator.evaluate(&expression(sql)),
            Err(EvalError::UnsupportedExpression)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 100);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    assert_eq!(
        evaluator.evaluate(&expression("Nz(Null)")),
        Ok(Value::Text(String::new()))
    );
    assert!(
        matches!(evaluator.evaluate(&expression("Nz(1,'unused')")), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
    );
}

#[test]
fn cancelled_nz_does_not_resolve_operands() {
    let bindings = Ordered {
        values: [Value::Null, Value::Int16(7), Value::Int32(9)],
        calls: RefCell::new(vec![]),
    };
    let mut budget = ResourceBudget::new(1024, 0, 100);
    assert_eq!(
        Evaluator::new(&bindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .evaluate(&expression("Nz([yes],[no])")),
        Err(EvalError::Cancelled)
    );
    assert!(bindings.calls.borrow().is_empty());
}

#[test]
fn numeric_calls_use_closed_dispatch_and_preserve_eager_argument_errors() {
    let mut budget = ResourceBudget::new(4096, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (sql, expected) in [
        ("Abs(CInt(-7))", Value::Int16(7)),
        ("Sgn(CDec(-0.00001))", Value::Int16(-1)),
        (
            "Int(CDec(-8.4))",
            Value::Decimal(tablecraft_core::values::Decimal::new(-9, 0).unwrap()),
        ),
        (
            "Fix(CDec(-8.4))",
            Value::Decimal(tablecraft_core::values::Decimal::new(-8, 0).unwrap()),
        ),
        (
            "Round(CDec(1.25),1)",
            Value::Decimal(tablecraft_core::values::Decimal::new(12, 1).unwrap()),
        ),
    ] {
        assert_eq!(evaluator.evaluate(&expression(sql)), Ok(expected), "{sql}");
    }
    assert_eq!(
        evaluator.evaluate(&expression("Round(NULL,1/0)")),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        evaluator.evaluate(&expression("Round(1,2,3)")),
        Err(EvalError::UnsupportedExpression)
    );
}
