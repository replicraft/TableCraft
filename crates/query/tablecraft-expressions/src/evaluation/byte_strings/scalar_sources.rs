use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn expression(sql: &str) -> Expression {
    let parsed = parse_select(&format!("SELECT {sql}"), Default::default()).unwrap();
    let Projection::Expression { expression, .. } = parsed.query().projection[0].clone() else {
        panic!("expected scalar expression");
    };
    expression
}

fn run(sql: &str, format: Option<NumberFormat>) -> Result<Value, EvalError> {
    let mut budget = ResourceBudget::new(100_000, 0, 100_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_date_format(DateTextFormat::Iso);
    if let Some(format) = format {
        evaluator = evaluator.with_number_format(format);
    }
    evaluator.evaluate(&expression(sql))
}

#[test]
fn scalar_byte_sources_share_explicit_cstr_and_utf16_profiles() {
    for (sql, expected) in [
        ("LeftB(1234,4)", Value::Text("12".into())),
        ("RightB(1234,4)", Value::Text("34".into())),
        ("MidB(1234,3,4)", Value::Text("23".into())),
        ("AscB(1234)", Value::Int16(49)),
        ("InStrB(1,1234,23,0)", Value::Int32(3)),
        ("StrConv(1234,0)", Value::Text("1234".into())),
        ("StrConv(True,1,1033)", Value::Text("TRUE".into())),
        (
            "StrConv(123,64,1033)",
            Value::Text("1\u{0000}2\u{0000}3\u{0000}".into()),
        ),
        (
            "StrConv(StrConv(1234,128,1033),64,1033)",
            Value::Text("1234".into()),
        ),
        ("MidB(#1900-01-01#,1,8)", Value::Text("1900".into())),
    ] {
        assert_eq!(run(sql, Some(NumberFormat::EnUs)), Ok(expected), "{sql}");
    }
    for (format, separator) in [(NumberFormat::EnUs, "."), (NumberFormat::DeDe, ",")] {
        assert_eq!(
            run("MidB(CCur(12.5),5,2)", Some(format)),
            Ok(Value::Text(separator.into()))
        );
        assert_eq!(run("InStrB(1,12.5,5,0)", Some(format)), Ok(Value::Int32(7)));
    }
}

#[test]
fn conversion_does_not_erase_null_validation_or_raw_publication_boundaries() {
    for sql in [
        "LeftB(123,2)",
        "RightB(123,2)",
        "MidB(123,1)",
        "AscB(123)",
        "InStrB(1,123,2,0)",
        "StrConv(123,0)",
    ] {
        assert_eq!(
            run(sql, None),
            Err(EvalError::NumberFormatRequired),
            "{sql}"
        );
    }
    for sql in [
        "LeftB(NULL,2)",
        "InStrB(1,NULL,123,0)",
        "StrConv(NULL,1,1033)",
    ] {
        assert_eq!(run(sql, None), Ok(Value::Null), "{sql}");
    }
    for sql in [
        "LeftB(123,1/0)",
        "StrConv(123,1,1/0)",
        "InStrB(1,NULL,1/0,0)",
    ] {
        assert_eq!(run(sql, None), Err(EvalError::DivisionByZero), "{sql}");
    }
    assert_eq!(run("LeftB(123,-1)", None), Err(EvalError::Domain));
    assert_eq!(run("InStrB(0,123,2,0)", None), Err(EvalError::Domain));
    assert_eq!(
        run("StrConv(123,4,1033)", None),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        run("LeftB(123,1)", Some(NumberFormat::EnUs)),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        run("AscB(LeftB(123,1))", Some(NumberFormat::EnUs)),
        Ok(Value::Int16(49))
    );
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_byte_string(
                TextFunction::LeftB,
                ByteOperand::Scalar(Value::Binary(vec![65])),
                &[Value::Int64(1)]
            )
            .unwrap_err(),
        EvalError::TypeMismatch
    );
}

#[test]
fn scalar_conversion_and_byte_buffers_share_limits_and_every_cancellation_point() {
    let expression = expression("StrConv(LeftB(12345,8),1,1033)");
    let calls = Cell::new(0);
    let invoke = |stop, bytes, text_bytes| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(bytes, 0, 10_000);
        Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: text_bytes,
                ..Default::default()
            },
        )
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&cancelled)
        .evaluate(&expression)
    };
    assert_eq!(
        invoke(usize::MAX, 1000, 1000),
        Ok(Value::Text("1234".into()))
    );
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop, 1000, 1000), Err(EvalError::Cancelled));
    }
    assert!(invoke(usize::MAX, 5, 1000).is_err());
    assert_eq!(invoke(usize::MAX, 1000, 4), Err(EvalError::TextLimit));
}
