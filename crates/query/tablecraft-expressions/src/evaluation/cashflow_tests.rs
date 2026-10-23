use super::*;
use crate::{
    EvalError,
    functions::{conversions::text::NumberFormat, financial::cashflows::CashFlowFunction},
};
use std::cell::{Cell, RefCell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, Value},
};
use tablecraft_sql::{
    parse_select,
    select::{Expression, FieldReference, Projection},
};

fn expression(text: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {text}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expected expression");
    };
    expression
}

fn evaluate(text: &str) -> Result<Value, EvalError> {
    Evaluator::new(
        &NoBindings,
        &mut ResourceBudget::new(65_536, 0, 10_000),
        Default::default(),
    )
    .evaluate(&expression(text))
}

#[test]
fn explicit_arrays_are_consumed_without_becoming_stored_scalars() {
    for (sql, expected) in [
        ("NPV(0,Array(-100,0,121))", 21.),
        ("NPV(0,VBA.Array(1,2,3))", 6.),
        ("IRR(Array(-100,0,121),)", 0.1),
        ("MIRR(Array(-100,0,121),0,0)", 0.1),
    ] {
        let Value::Double(actual) = evaluate(sql).unwrap() else {
            panic!("expected Double");
        };
        assert!((actual.to_float() - expected).abs() < 1e-11, "{sql}");
    }

    for sql in [
        "Array(1,2)",
        "NPV(0,1,2)",
        "NPV(0,Null)",
        "IRR(Array(-100,,121))",
        "IRR(Array(Array(1),2))",
        "IRR(Other.Array(-100,121))",
        "NPV(Array(1,2),0)",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
    assert_eq!(evaluate("NPV(0,Array())"), Err(EvalError::Domain));
    assert_eq!(
        evaluate("IRR(Array(-100,121),Null)"),
        Err(EvalError::InvalidNullConversion)
    );
}

#[test]
fn all_written_expressions_precede_conversion_and_keep_their_order() {
    struct Ordered(RefCell<Vec<String>>);

    impl Bindings for Ordered {
        fn resolve(&self, field: &FieldReference) -> Result<&Value, EvalError> {
            self.0.borrow_mut().push(field.name.as_str().into());
            Ok(&Value::Int32(1))
        }
    }

    for (sql, expected) in [
        ("NPV(R,Array(A,B))", vec!["R", "A", "B"]),
        ("IRR(Array(A,B),G)", vec!["A", "B", "G"]),
        ("MIRR(Array(A,B),F,R)", vec!["A", "B", "F", "R"]),
    ] {
        let bindings = Ordered(RefCell::new(vec![]));
        let mut budget = ResourceBudget::new(65_536, 0, 10_000);
        let _ =
            Evaluator::new(&bindings, &mut budget, Default::default()).evaluate(&expression(sql));
        assert_eq!(*bindings.0.borrow(), expected);
    }

    for sql in [
        "NPV(Null,Array(1,1/0))",
        "IRR(Array(Null,1/0))",
        "MIRR(Array(Null,1),0,1/0)",
        "IIf(False,IRR(Array(-1,1/0)),1)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::DivisionByZero), "{sql}");
    }
}

#[test]
fn array_constructor_depth_allocation_and_work_are_charged() {
    let expr = expression("NPV(0,Array(1))");
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_depth: 1,
                ..Default::default()
            }
        )
        .evaluate(&expr),
        Err(EvalError::DepthLimit)
    );

    let mut budget = ResourceBudget::new(0, 0, 10_000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expr),
        Err(EvalError::Limit(error)) if error.resource == Resource::Bytes
    ));
    let mut budget = ResourceBudget::new(65_536, 0, 10_000);
    let actual = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .evaluate(&expr)
        .unwrap();
    assert_eq!(actual, Value::Double(Double::from_float(1.)));
    assert_eq!(budget.remaining(Resource::Rows), 0);
    assert!(budget.remaining(Resource::Steps) < 10_000 - 3);
}

#[test]
fn cashflow_rate_text_uses_explicit_policy_and_keeps_array_conversion_order() {
    for (text, numeric) in [
        ("NPV('0,1',Array(-100,0,121))", "NPV(0.1,Array(-100,0,121))"),
        ("IRR(Array(-100,0,121),'0,1')", "IRR(Array(-100,0,121),0.1)"),
        (
            "MIRR(Array(-100,0,121),'0,05','0,1')",
            "MIRR(Array(-100,0,121),0.05,0.1)",
        ),
    ] {
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let value = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::DeDe)
            .evaluate(&expression(text));
        assert_eq!(value, evaluate(numeric), "{text}");
        assert_eq!(
            evaluate(text),
            Err(EvalError::NumberFormatRequired),
            "{text}"
        );
    }
    for (sql, error) in [
        ("NPV('bad',Array(NULL))", EvalError::NumberFormatRequired),
        ("IRR(Array(NULL),'bad')", EvalError::InvalidNullConversion),
        (
            "MIRR(Array(NULL),'bad','bad')",
            EvalError::InvalidNullConversion,
        ),
        ("NPV('bad',Array(1/0))", EvalError::DivisionByZero),
        ("IRR(Array('-100','121'),'0,1')", EvalError::TypeMismatch),
    ] {
        assert_eq!(evaluate(sql), Err(error), "{sql}");
    }
}

#[test]
fn cashflow_text_conversion_shares_limits_and_every_observed_cancellation_checkpoint() {
    for sql in [
        "NPV('0.25',Array(-100,0,125))",
        "IRR(Array(-100,0,121),'0.1')",
        "MIRR(Array(-100,0,121),'0','0')",
    ] {
        let expression = expression(sql);
        let checkpoints = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            })
            .evaluate(&expression)
            .unwrap();
        for stop in 1..=checkpoints.get() {
            let calls = Cell::new(0);
            let mut budget = ResourceBudget::new(65536, 0, 10000);
            let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() >= stop
                })
                .evaluate(&expression);
            assert_eq!(result, Err(EvalError::Cancelled), "{sql} checkpoint {stop}");
        }
    }
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 2,
                ..Default::default()
            }
        )
        .with_number_format(NumberFormat::EnUs)
        .apply_cashflows(
            CashFlowFunction::NPV,
            &[Value::Text("0.25".into())],
            &[Value::Int16(1)]
        ),
        Err(EvalError::TextLimit)
    );
}
