use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{parse_select, select::Projection};

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
fn nested_byte_producers_retain_raw_lengths_codes_and_string_subtypes() {
    for (sql, expected) in [
        ("LenB(ChrB(255))", Value::Int32(1)),
        ("AscB(ChrB(255))", Value::Int16(255)),
        ("AscB(ChrB(0))", Value::Int16(0)),
        ("AscB(ChrB(254.5))", Value::Int16(254)),
        ("LenB(LeftB('🦀',3))", Value::Int32(3)),
        ("AscB(RightB('🦀',3))", Value::Int16(216)),
        ("AscB(MidB('中',2,1))", Value::Int16(78)),
        ("LenB(MidB('A',2))", Value::Int32(1)),
        ("MidB('🦀',2,2)", Value::Text("\u{80d8}".into())),
        ("LenB(MidB('A',99))", Value::Int32(0)),
        ("LenB(LeftB(Null,2))", Value::Null),
        ("LenB(CVar(MidB('🦀',2,2)))", Value::Int32(2)),
        ("TypeName(ChrB(65))", Value::Text("String".into())),
        ("VarType(CVar(ChrB(65)))", Value::Int16(8)),
        ("IsArray(ChrB(65))", Value::Boolean(false)),
        ("IsEmpty(ChrB(65))", Value::Boolean(false)),
        ("IsError(ChrB(65))", Value::Boolean(false)),
        ("LeftB('A中🦀',4)", Value::Text("A中".into())),
        ("MidB('A中🦀',3,6)", Value::Text("中🦀".into())),
        ("RightB('A中🦀',4)", Value::Text("🦀".into())),
        ("LeftB(ChrB(65),0)", Value::Text(String::new())),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn byte_producers_have_strict_publication_shapes_and_eager_failures() {
    for sql in [
        "ChrB(65)",
        "LeftB('A',1)",
        "RightB('🦀',2)",
        "MidB('🦀',1,2)",
        "CVar(ChrB(65))",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnrepresentableText), "{sql}");
    }
    for sql in [
        "LeftB('x',1,1/0)",
        "ChrB(1,1/0)",
        "MidB('x',)",
        "MidB('x',1,)",
        "LeftB(*)",
    ] {
        assert_eq!(
            evaluate(sql),
            Err(EvalError::UnsupportedExpression),
            "{sql}"
        );
    }
    for sql in [
        "LeftB(Null,1/0)",
        "MidB('x',-1,1/0)",
        "LeftB(ChrB(999),1/0)",
    ] {
        let expected = if sql.contains("999") {
            EvalError::Overflow
        } else {
            EvalError::DivisionByZero
        };
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
    for sql in ["ChrB(-1)", "ChrB(256)"] {
        assert_eq!(evaluate(sql), Err(EvalError::Overflow));
    }
    for sql in ["LeftB('x',-1)", "MidB('x',0)", "MidB('x',100,-1)"] {
        assert_eq!(evaluate(sql), Err(EvalError::Domain));
    }
    assert_eq!(evaluate("LBound(ChrB(65))"), Err(EvalError::TypeMismatch));
    assert_eq!(
        evaluate("ChrB(Null)"),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(evaluate("LenB(LeftB(123,1))"), Err(EvalError::TypeMismatch));
}

#[test]
fn byte_allocations_publication_and_every_checkpoint_share_host_limits() {
    for (sql, bytes) in [("LenB(ChrB(65))", 0), ("LenB(LeftB('A',1))", 2)] {
        let mut budget = ResourceBudget::new(bytes, 0, 1000);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(&expression(sql)), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
    }
    let mut budget = ResourceBudget::new(100, 0, 1000);
    let raw = ByteString::from_owned(vec![65, 0, 66, 0]).unwrap();
    assert_eq!(
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 1,
                ..Default::default()
            }
        )
        .publish_byte_operand(ByteOperand::Bytes(raw)),
        Err(EvalError::TextLimit)
    );

    let expr = expression("LenB(CVar(RightB(MidB('A中🦀',2),3)))");
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_cancellation(&count)
        .evaluate(&expr)
        .unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let cancel = || {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&cancel)
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
}

#[test]
fn instrb_uses_raw_byte_operands_and_explicit_binary_policy() {
    for (sql, expected) in [
        ("InStrB(1,'A中🦀','🦀',0)", Value::Int32(5)),
        ("InStrB(1,'A',ChrB(0),0)", Value::Int32(2)),
        ("InStrB(2,'A',ChrB(65),0)", Value::Int32(0)),
        ("InStrB(1,ChrB(65),ChrB(65),0)", Value::Int32(1)),
        ("InStrB(1,RightB('🦀',3),ChrB(216),0)", Value::Int32(1)),
        ("InStrB(1,'',ChrB(65),0)", Value::Int32(0)),
        ("InStrB(2,'A','',0)", Value::Int32(2)),
        ("InStrB(3,'A','',0)", Value::Int32(0)),
        ("InStrB(1,Null,'A',0)", Value::Null),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
    assert_eq!(
        evaluate("InStrB('A','A')"),
        Err(EvalError::CollationRequired)
    );
    for sql in [
        "InStrB(1,'A','a',1)",
        "InStrB(1,'A','A',2)",
        "InStrB(1,'A','A',1033)",
    ] {
        assert_eq!(evaluate(sql), Err(EvalError::UnsupportedExpression));
    }
    for sql in ["InStrB(Null,'A','A',0)", "InStrB(1,'A','A',Null)"] {
        assert_eq!(evaluate(sql), Err(EvalError::InvalidNullConversion));
    }
    for sql in ["InStrB(0,'A','A',0)", "InStrB(1,'A','A',-2)"] {
        assert_eq!(evaluate(sql), Err(EvalError::Domain));
    }
    for sql in ["InStrB(1,Null,'A',1/0)", "InStrB(1,ChrB(1),ChrB(1/0),0)"] {
        assert_eq!(evaluate(sql), Err(EvalError::DivisionByZero));
    }
}
