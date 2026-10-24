use super::*;
use crate::{
    comparison::{Collation, StringCompareMode},
    evaluation::{EvalLimits, NoBindings},
    functions::{
        math::random::RndState,
        text::byte_string::{ByteString, ExpressionByteOperand},
    },
};
use std::cell::Cell;
use std::cmp::Ordering;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::{parse_select, select::Projection};

fn text(value: &str) -> ExpressionValue {
    ExpressionValue::Value(Value::Text(value.into()))
}

fn mode(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn adopted_binary_comparison_requires_no_payload_budget_or_provider_callback() {
    struct BinaryOption;
    impl Collation for BinaryOption {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            panic!("binary comparison must not call the Unicode provider");
        }

        fn option_compare(&self) -> Option<StringCompareMode> {
            Some(StringCompareMode::Binary)
        }
    }

    let mut budget = ResourceBudget::new(0, 0, 100);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, Default::default()).with_collation(&BinaryOption);
    let raw = || ExpressionByteOperand::Bytes(ByteString::from_owned(vec![0, 216]).unwrap());
    assert_eq!(
        evaluator.apply_expression_byte_comparison(raw(), raw(), None),
        Ok(Value::Int16(0))
    );
    assert_eq!(
        evaluator.apply_expression_byte_comparison(
            ExpressionByteOperand::Bytes(ByteString::from_owned(vec![65]).unwrap()),
            ExpressionByteOperand::Scalar(ExpressionValue::Value(Value::Null)),
            None,
        ),
        Ok(Value::Null)
    );
}

#[test]
fn binary_comparison_consumes_raw_units_without_unicode_publication() {
    for (sql, expected) in [
        ("StrComp(ChrW(55296),ChrW(55297),0)", Value::Int16(-1)),
        ("StrComp(ChrW(55296),ChrW(55296),0)", Value::Int16(0)),
        ("StrComp(ChrW(57343),'A',0)", Value::Int16(1)),
        ("StrComp(ChrB(65),NULL,0)", Value::Null),
        ("StrComp(MidB('😀',2,2),ChrW(216),0)", Value::Int16(0)),
    ] {
        let parsed = parse_select(&format!("SELECT {sql}"), Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result =
            Evaluator::new(&NoBindings, &mut budget, Default::default()).evaluate(expression);

        assert_eq!(result, Ok(expected), "{sql}");
    }
}

#[test]
fn comparison_states_keep_mode_null_and_progressive_string_admission() {
    let error = ExpressionValue::Error(11);
    let null = ExpressionValue::Value(Value::Null);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());

    for (arguments, expected) in [
        (
            vec![ExpressionValue::Empty, text(""), ExpressionValue::Empty],
            Ok(Value::Int16(0)),
        ),
        (
            vec![ExpressionValue::Empty, ExpressionValue::Empty],
            Err(EvalError::CollationRequired),
        ),
        (vec![null.clone(), error.clone(), mode(0)], Ok(Value::Null)),
        (vec![error.clone(), null.clone(), mode(0)], Ok(Value::Null)),
        (
            vec![null.clone(), ExpressionValue::Missing, mode(0)],
            Ok(Value::Null),
        ),
        (
            vec![null.clone(), error.clone(), mode(3)],
            Err(EvalError::Domain),
        ),
        (
            vec![null, text(""), error.clone()],
            Err(EvalError::TypeMismatch),
        ),
        (
            vec![mode(123), error.clone(), mode(0)],
            Err(EvalError::NumberFormatRequired),
        ),
        (vec![text(""), error, mode(0)], Err(EvalError::TypeMismatch)),
        (
            vec![text(""), ExpressionValue::Missing, mode(0)],
            Err(EvalError::NonScalarExpression),
        ),
    ] {
        assert_eq!(evaluator.apply_expression_comparison(&arguments), expected);
    }

    assert_eq!(
        evaluator.apply_expression_comparison(&[]),
        Err(EvalError::UnsupportedExpression)
    );
}

#[test]
fn comparison_states_preserve_limits_and_every_observed_cancellation_checkpoint() {
    let arguments = [text("banana"), text("band"), ExpressionValue::Empty];
    let calls = Cell::new(0);
    let invoke = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&cancelled)
            .apply_expression_comparison(&arguments)
    };
    assert_eq!(invoke(usize::MAX), Ok(Value::Int16(-1)));
    let total = calls.get();
    for stop in 1..=total {
        assert_eq!(invoke(stop), Err(EvalError::Cancelled));
    }

    let mut budget = ResourceBudget::new(65536, 0, 0);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_expression_comparison(&arguments),
        Err(EvalError::Limit(_))
    ));

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    let limits = EvalLimits {
        max_text_bytes: 3,
        ..EvalLimits::default()
    };
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, limits).apply_expression_comparison(&[
            text("over"),
            ExpressionValue::Value(Value::Null),
            mode(0),
        ]),
        Err(EvalError::TextLimit)
    );
}

#[test]
fn comparison_sql_evaluates_late_operands_once_before_state_admission() {
    for (sql, error) in [
        ("SELECT StrComp(CVErr(11),'a',3)", EvalError::Domain),
        (
            "SELECT StrComp(CVErr(11),'a',1/0)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT StrComp(NULL,CStr(NULL),0)",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT StrComp(123,CVErr(11),0)",
            EvalError::NumberFormatRequired,
        ),
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(expression),
            Err(error)
        );
    }

    let parsed = parse_select("SELECT StrComp(CVErr(11),Rnd(),0)", Default::default()).unwrap();
    let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
        panic!("expression required");
    };
    let state = RndState::default();
    let expected = RndState::default();
    expected.apply(None).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_random(&state)
            .evaluate(expression),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(state.apply(None), expected.apply(None));
}
