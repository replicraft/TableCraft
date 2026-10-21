use super::*;
use crate::EvalError;
use std::cell::RefCell;
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
