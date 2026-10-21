use super::*;
use crate::{
    EvalError,
    functions::inspection::types::{InspectionInput, ValueInspectionFunction},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
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
fn inspection_and_bounds_consume_explicit_arrays_and_preserve_result_widths() {
    for (sql, expected) in [
        ("TypeName(CInt(1))", Value::Text("Integer".into())),
        ("TypeName(CLng(1))", Value::Text("Long".into())),
        ("TypeName(CLngLng(1))", Value::Text("LongLong".into())),
        ("VarType(CCur(1))", Value::Int16(6)),
        ("VarType(CDec(1))", Value::Int16(14)),
        ("TypeName(Null)", Value::Text("Null".into())),
        ("IsEmpty(Null)", Value::Boolean(false)),
        ("IsEmpty('')", Value::Boolean(false)),
        ("IsError(1)", Value::Boolean(false)),
        ("IsArray(Null)", Value::Boolean(false)),
        ("IsArray(Array())", Value::Boolean(true)),
        ("TypeName(VBA.Array(1,2))", Value::Text("Variant()".into())),
        ("VarType(Array(Null,'secret'))", Value::Int16(8204)),
        ("LBound(Array(1,2),)", Value::Int32(0)),
        ("UBound(VBA.Array(1,2,Null))", Value::Int32(2)),
        ("UBound(Array())", Value::Int32(-1)),
        ("LBound(Array())", Value::Int32(0)),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn type_inspection_does_not_catch_errors_or_hide_array_elements() {
    for sql in [
        "IsError(1/0)",
        "TypeName(Array(Null,1/0))",
        "VarType(Array(1/0))",
        "IsArray(Array(1/0))",
        "IsEmpty(Array(1/0))",
        "IsError(Array(1/0))",
        "LBound(Array(1/0))",
        "UBound(Null,1/0)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::DivisionByZero), "{sql}");
    }

    for sql in [
        "IsArray(Array(,))",
        "VarType(Array(Array(1)))",
        "TypeName(Other.Array(1))",
        "Array(1)",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
    assert_eq!(evaluate("UBound(Array(1),2)"), Err(EvalError::ArrayBounds));
    assert_eq!(
        evaluate("UBound(Array(1),Null)"),
        Err(EvalError::InvalidNullConversion)
    );
}

#[test]
fn names_constructor_buffers_work_and_cancellation_use_the_existing_budget() {
    let value = Value::Int16(1);
    let mut budget = ResourceBudget::new(6, 0, 100);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_value_inspection(ValueInspectionFunction::TypeName, InspectionInput::Scalar(&value)),
        Err(EvalError::Limit(error)) if error.resource == Resource::Bytes
    ));

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
        .apply_value_inspection(
            ValueInspectionFunction::TypeName,
            InspectionInput::Scalar(&value)
        ),
        Err(EvalError::TextLimit)
    );

    let expr = expression("VarType(Array(1,2))");
    for (bytes, steps, cancelled) in [(0, 100, false), (1000, 2, false), (1000, 100, true)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        let error = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&|| cancelled)
            .evaluate(&expr)
            .unwrap_err();
        assert!(matches!(error, EvalError::Limit(_) | EvalError::Cancelled));
    }

    let mut budget = ResourceBudget::new(1000, 0, 100);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .evaluate(&expr)
        .unwrap();
    assert_eq!(result, Value::Int16(8204));
    assert_eq!(budget.remaining(Resource::Rows), 0);
}

#[test]
fn bound_binary_inspection_borrows_bytes_and_resolves_the_field_once() {
    struct BinaryBinding {
        value: Value,
        calls: Cell<usize>,
    }

    impl Bindings for BinaryBinding {
        fn resolve(&self, _field: &FieldReference) -> Result<&Value, EvalError> {
            self.calls.set(self.calls.get() + 1);
            Ok(&self.value)
        }
    }

    let bindings = BinaryBinding {
        value: Value::Binary(vec![0xff; 70_000]),
        calls: Cell::new(0),
    };
    for (sql, expected) in [
        ("IsArray(B)", Value::Boolean(true)),
        ("VarType(B)", Value::Int16(8209)),
        ("UBound(B)", Value::Int32(69_999)),
    ] {
        bindings.calls.set(0);
        let mut budget = ResourceBudget::new(0, 0, 100);
        let result = Evaluator::new(&bindings, &mut budget, Default::default())
            .evaluate(&expression(sql))
            .unwrap();

        assert_eq!(result, expected);
        assert_eq!(bindings.calls.get(), 1);
        assert_eq!(budget.remaining(Resource::Bytes), 0);
    }
}
