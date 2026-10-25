use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    parse_select,
    select::{Expression, Projection},
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
fn strconv_casing_encoding_and_nested_byte_results_retain_representation() {
    for (sql, expected) in [
        (
            "StrConv('Hello WORLD',1,1033)",
            Value::Text("HELLO WORLD".into()),
        ),
        (
            "StrConv('Hello WORLD',2,1033)",
            Value::Text("hello world".into()),
        ),
        (
            "StrConv('hELLO WORLD',3,1033)",
            Value::Text("Hello World".into()),
        ),
        ("StrConv('Iİiı',2,1055)", Value::Text("ıiiı".into())),
        ("StrConv('ijSSEL',3,1043)", Value::Text("IJssel".into())),
        ("StrConv('A',64)", Value::Text("A\0".into())),
        ("StrConv(ChrB(128),64,1033)", Value::Text("€".into())),
        ("StrConv(ChrB(97),65,1033)", Value::Text("A".into())),
        (
            "StrConv(StrConv('é€',128,1033),64,1033)",
            Value::Text("é€".into()),
        ),
        ("AscB(StrConv('é',129,1033))", Value::Int16(201)),
        ("AscB(StrConv('I',130,1055))", Value::Int16(253)),
        ("LenB(StrConv('€',128,1033))", Value::Int32(1)),
        ("VarType(StrConv('€',128,1033))", Value::Int16(8)),
        ("IsArray(StrConv('€',128,1033))", Value::Boolean(false)),
        ("LenB(StrConv(ChrB(65),0))", Value::Int32(1)),
        ("StrConv(Null,1)", Value::Null),
        ("StrConv('a',1,1041)", Value::Text("A".into())),
        (
            "StrConv(StrConv('a日本',129,1041),64,1041)",
            Value::Text("A日本".into()),
        ),
        (
            "StrConv(StrConv('a日本',128,1041),65,1041)",
            Value::Text("A日本".into()),
        ),
    ] {
        assert_eq!(evaluate(sql), Ok(expected), "{sql}");
    }
}

#[test]
fn strconv_refuses_unsupported_requests_and_keeps_eager_error_order() {
    for (sql, expected) in [
        ("StrConv('x',1)", EvalError::CaseLocaleRequired),
        ("StrConv('x',4,1033)", EvalError::UnsupportedExpression),
        ("StrConv('x',1,1042)", EvalError::UnsupportedExpression),
        ("StrConv('x',1,66569)", EvalError::UnsupportedExpression),
        ("StrConv('x',192)", EvalError::Domain),
        ("StrConv('x',256)", EvalError::Domain),
        ("StrConv('x',Null)", EvalError::InvalidNullConversion),
        ("StrConv('x',1,Null)", EvalError::InvalidNullConversion),
        ("StrConv('x',1,0)", EvalError::Domain),
        ("StrConv('x',1,-1)", EvalError::Domain),
        ("StrConv('🦀',128,1033)", EvalError::UnmappableCharacter),
        (
            "StrConv(ChrB(170),64,1032)",
            EvalError::InvalidCharacterCode,
        ),
        ("StrConv('€',128,1033)", EvalError::UnrepresentableText),
        ("StrConv(ChrB(65),1,1033)", EvalError::UnrepresentableText),
        ("StrConv(ChrB(65),0)", EvalError::UnrepresentableText),
        ("StrConv(123,1,1033)", EvalError::NumberFormatRequired),
        ("StrConv(Null,1,1/0)", EvalError::DivisionByZero),
        ("StrConv('x',4,1/0)", EvalError::DivisionByZero),
        ("StrConv('x',1,1033,1/0)", EvalError::UnsupportedExpression),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }
}

#[test]
fn strconv_metering_covers_encoding_casing_output_and_every_cancellation_point() {
    let expr = expression("StrConv(StrConv('Hello é',129,1033),66,1033)");
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 10000);
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
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_cancellation(&cancel)
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert!(
        matches!(Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_strconv(ByteOperand::Scalar(Value::Text("A".into())), &Value::Int16(128), None), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
    );
}

#[test]
fn strconv_output_limits_reject_expanding_case_and_byte_decoding() {
    for (input, conversion, lcid, limit) in [
        (ByteOperand::Scalar(Value::Text("i".into())), 1, 1055, 1),
        (
            ByteOperand::Bytes(ByteString::from_owned(vec![128]).unwrap()),
            64,
            1033,
            2,
        ),
        (ByteOperand::Scalar(Value::Text("AB".into())), 0, 1033, 1),
    ] {
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        let mut evaluator = Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: limit,
                ..Default::default()
            },
        );
        let result =
            evaluator.apply_strconv(input, &Value::Int16(conversion), Some(&Value::Int32(lcid)));
        assert!(matches!(result, Err(EvalError::TextLimit)));
    }
}

#[test]
fn contextual_options_preserve_flag_range_locale_validation_and_null_precedence() {
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs);
    for (flag, locale, expected) in [
        ("32767.5", "1033", EvalError::Overflow),
        ("5", "bad", EvalError::TypeMismatch),
        ("100", "bad", EvalError::UnsupportedExpression),
        ("4", "bad", EvalError::TypeMismatch),
        ("0", "bad", EvalError::TypeMismatch),
        ("0", "0", EvalError::Domain),
        ("0", "1042", EvalError::UnsupportedExpression),
    ] {
        let result = evaluator.apply_strconv(
            ByteOperand::Scalar(Value::Null),
            &Value::Text(flag.into()),
            Some(&Value::Text(locale.into())),
        );
        assert!(matches!(result, Err(error) if error == expected));
    }
    let result = evaluator
        .apply_strconv(
            ByteOperand::Scalar(Value::Null),
            &Value::Text("1.5".into()),
            Some(&Value::Text("1033".into())),
        )
        .unwrap();
    assert!(matches!(result, ByteOperand::Scalar(Value::Null)));
}

#[test]
fn normalization_budgets_and_all_contextual_option_checkpoints_are_shared() {
    let mut budget = ResourceBudget::new(8, 0, 1000);
    let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .apply_strconv(
            ByteOperand::Scalar(Value::Text("I".into())),
            &Value::Text("1.5".into()),
            Some(&Value::Text("1033".into())),
        )
        .unwrap();
    assert!(matches!(result, ByteOperand::Scalar(Value::Text(value)) if value == "i"));
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    let expr = expression("StrConv('Hello','1.5','1033')");
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&callback)
        .evaluate(&expr)
        .unwrap();
    for stop in 1..=observed.get() {
        let current = Cell::new(0);
        let callback = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&callback)
                .evaluate(&expr),
            Err(EvalError::Cancelled)
        );
    }
}
