use super::{Evaluator, NoBindings};
use crate::{EvalError, functions::conversions::text::NumberFormat};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
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
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression(text))
}

#[test]
fn error_data_survives_selection_and_explicit_consumers_without_catching_failures() {
    for (sql, expected) in [
        ("IsError(CVErr(2001))", Value::Boolean(true)),
        ("VarType(CVar(CVErr(2001)))", Value::Int16(10)),
        ("TypeName(CVErr(0))", Value::Text("Error".into())),
        ("IsArray(CVErr(2001))", Value::Boolean(false)),
        ("IsEmpty(CVErr(2001))", Value::Boolean(false)),
        ("IsNull(CVErr(0))", Value::Boolean(false)),
        ("CVErr(0) IS NOT NULL", Value::Boolean(true)),
        ("CLng(CVErr(CVErr(65535)))", Value::Int32(65535)),
        ("CInt(CVErr('2,001'))", Value::Int16(2001)),
        ("IIf(False,CVErr(2001),1)", Value::Int64(1)),
        ("IsError(IIf(True,CVErr(2001),0))", Value::Boolean(true)),
        ("IsError(Nz(CVErr(2001),0))", Value::Boolean(true)),
        ("IsError(Nz(Null,CVErr(2001)))", Value::Boolean(true)),
        ("IsError(Choose(2,0,CVErr(2001)))", Value::Boolean(true)),
        (
            "IsError(Switch(False,CVErr(1),True,CVErr(2)))",
            Value::Boolean(true),
        ),
        ("Switch(True,1,False,CVErr(2001))", Value::Int64(1)),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .evaluate_expression_value(&expression("IIf(True,CVErr(2001),0)")),
        Ok(ExpressionValue::Error(2001))
    );
    for (sql, expected) in [
        ("CVErr(2001)", EvalError::NonScalarExpression),
        ("CVErr(2001)+1", EvalError::TypeMismatch),
        ("IsError(1/0)", EvalError::DivisionByZero),
        ("IsError(IIf(CVErr(1),0,1/0))", EvalError::DivisionByZero),
        ("IsError(IIf(CVErr(1),0,1))", EvalError::TypeMismatch),
        ("IsError(CVErr(-1))", EvalError::Domain),
        ("CInt(CVErr(65535))", EvalError::Overflow),
        (
            "IsError(IIf(True,CVErr(2001),1/0))",
            EvalError::DivisionByZero,
        ),
        (
            "IsError(Switch(True,CVErr(2001),False,1/0))",
            EvalError::DivisionByZero,
        ),
        (
            "IsError(Choose(1,CVErr(2001),1/0))",
            EvalError::DivisionByZero,
        ),
        ("CVErr(1,1/0)", EvalError::UnsupportedExpression),
        ("CVErr(*)", EvalError::UnsupportedExpression),
        ("LBound(IIf(True,Null,0))", EvalError::InvalidNullConversion),
        ("IsArray(CVar(Array(1)))", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn every_observed_checkpoint_and_text_conversion_limits_remain_enforced() {
    let expr = expression("CInt(IIf(True,CVErr('2,001'),CVErr(0)))");
    let count = Cell::new(0);
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&|| {
            count.set(count.get() + 1);
            false
        })
        .evaluate(&expr)
        .unwrap();
    let total = count.get();
    for stop in 1..=total {
        count.set(0);
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&|| {
                    count.set(count.get() + 1);
                    count.get() == stop
                })
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(1, 0, 10000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_number_format(NumberFormat::EnUs)
            .evaluate(&expr),
        Err(EvalError::Limit(_))
    ));
    let mut budget = ResourceBudget::new(1000, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expr),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn arithmetic_error_operands_fail_after_both_operands_evaluate() {
    for operator in ["+", "-", "*", "/", "\\", "Mod", "^"] {
        for (left, right) in [
            ("CVErr(0)", "1"),
            ("1", "CVErr(65535)"),
            ("CVErr(1)", "CVErr(2)"),
            ("CVErr(0)", "Null"),
            ("Null", "CVErr(0)"),
        ] {
            let sql = format!("{left} {operator} {right}");
            assert_eq!(evaluate(&sql), Err(EvalError::TypeMismatch), "{sql}");
        }
        for (left, right, error) in [
            ("CVErr(0)", "1/0", EvalError::DivisionByZero),
            ("1/0", "CVErr(0)", EvalError::DivisionByZero),
            ("CVErr(0)", "CVErr(-1)", EvalError::Domain),
        ] {
            let sql = format!("({left}) {operator} ({right})");
            assert_eq!(evaluate(&sql), Err(error), "{sql}");
        }
    }
    for sql in ["+CVErr(0)", "-CVErr(65535)", "IsError(CVErr(1)+1)"] {
        assert_eq!(evaluate(sql), Err(EvalError::TypeMismatch), "{sql}");
    }
    assert_eq!(evaluate("CLng(CVErr(1))+2"), Ok(Value::Int64(3)));
}

#[test]
fn standard_error_relations_lists_and_reversed_ranges_use_code_comparison() {
    for (sql, expected) in [
        ("CVErr(0)=CVErr(0)", true),
        ("CVErr(0)<>CVErr(65535)", true),
        ("CVErr(1)<CVErr(2)", true),
        ("CVErr(2)<=CVErr(2)", true),
        ("CVErr(65535)>CVErr(0)", true),
        ("CVErr(0)>=CVErr(65535)", false),
        ("CVErr(1) IN (CVErr(0),CVErr(1))", true),
        ("CVErr(1) NOT IN (CVErr(0),CVErr(1))", false),
        ("CVErr(1) BETWEEN CVErr(2) AND CVErr(0)", true),
        ("CVErr(1) NOT BETWEEN CVErr(2) AND CVErr(0)", false),
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Boolean(expected)), "{sql}");
    }
    for sql in [
        "CVErr(0)=0",
        "Null=CVErr(0)",
        "CVErr(0) IN (CVErr(0),Null)",
        "CVErr(0) BETWEEN CVErr(0) AND Null",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::TypeMismatch), "{sql}");
    }
    for sql in [
        "CVErr(0)=(1/0)",
        "CVErr(0) IN (CVErr(0),1/0)",
        "CVErr(0) BETWEEN CVErr(0) AND (1/0)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::DivisionByZero), "{sql}");
    }
}

#[test]
fn logical_error_operands_reject_after_eager_evaluation_instead_of_using_truth_tables() {
    for operator in ["And", "Or", "Xor", "Eqv", "Imp"] {
        for (left, right) in [
            ("CVErr(0)", "False"),
            ("True", "CVErr(1)"),
            ("Null", "CVErr(0)"),
            ("CVErr(0)", "CVErr(0)"),
        ] {
            let sql = format!("{left} {operator} {right}");
            assert_eq!(evaluate(&sql), Err(EvalError::TypeMismatch), "{sql}");
        }
        let sql = format!("CVErr(0) {operator} (1/0)");
        assert_eq!(evaluate(&sql), Err(EvalError::DivisionByZero), "{sql}");
    }
    assert_eq!(evaluate("Not CVErr(0)"), Err(EvalError::TypeMismatch));
}
