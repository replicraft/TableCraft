use super::{EvalLimits, Evaluator, NoBindings};
use crate::EvalError;
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, Value},
};
use tablecraft_sql::{
    parse_select,
    select::{Expression, Projection},
};

fn expression(text: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {text}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expression required");
    };
    expression
}

fn evaluate(text: &str) -> Result<Value, EvalError> {
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression(text))
}

#[test]
fn euro_optional_slots_remain_distinct_from_null_and_validate_before_evaluation() {
    for sql in [
        "EuroConvert(1.2,'DEM','EUR')",
        "EuroConvert(1.2,'DEM','EUR',)",
        "EuroConvert(1.2,'DEM','EUR',,)",
        "EuroConvert(1.2,'DEM','EUR',,3)",
    ] {
        assert_eq!(
            evaluate(sql),
            Ok(Value::Double(Double::from_float(0.61))),
            "{sql}"
        );
    }
    for (sql, error) in [
        (
            "EuroConvert(1,'DEM','EUR',Null)",
            EvalError::InvalidNullConversion,
        ),
        (
            "EuroConvert(1,'DEM','EUR',True,Null)",
            EvalError::InvalidNullConversion,
        ),
        ("EuroConvert(1,'DEM','EUR',True,0)", EvalError::Domain),
        ("EuroConvert(1,'DEM','EUR',True,2)", EvalError::Domain),
        (
            "EuroConvert(Null,'DEM','EUR',1/0)",
            EvalError::DivisionByZero,
        ),
        ("EuroConvert(1,'DEM','DEM',1/0)", EvalError::DivisionByZero),
        ("EuroConvert(1,'USD','EUR',1/0)", EvalError::DivisionByZero),
        ("EuroConvert(,1/0,'EUR')", EvalError::UnsupportedExpression),
        (
            "EuroConvert(1,'DEM','EUR',False,3,1/0)",
            EvalError::UnsupportedExpression,
        ),
        ("EuroConvert(*)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(error), "{sql}");
    }
}

#[test]
fn euro_checks_every_cancellation_checkpoint_and_keeps_resource_refusals() {
    let expr = expression("EuroConvert(1000,'FRF','DEM',True,3)");
    let checkpoints = Cell::new(0);
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&|| {
            checkpoints.set(checkpoints.get() + 1);
            false
        })
        .evaluate(&expr)
        .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&|| {
                    checkpoints.set(checkpoints.get() + 1);
                    checkpoints.get() == stop
                })
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
    for (bytes, steps, resource) in [(1, 10000, Resource::Bytes), (1000, 1, Resource::Steps)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expr), Err(EvalError::Limit(error)) if error.resource == resource)
        );
    }
}
