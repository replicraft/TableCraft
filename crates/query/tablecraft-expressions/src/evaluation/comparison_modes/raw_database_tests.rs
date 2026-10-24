use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringComparisonProfile},
    evaluation::{Evaluator, NoBindings},
    functions::{conversions::text::NumberFormat, text::byte_string::ExpressionByteOperand},
};
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_sql::parse_expression;

struct Reverse;

impl Collation for Reverse {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::CollationFailed)
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(right.cmp(left))
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
}

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_collation(&Reverse)
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression)
}

fn raw(bytes: Vec<u8>) -> ExpressionByteOperand {
    ExpressionByteOperand::Scalar(ExpressionValue::StringBytes(ExpressionString::from_owned(
        bytes,
    )))
}

#[test]
fn database_mode_uses_the_explicit_raw_provider_and_module_default() {
    for sql in [
        "StrComp(ChrW(55296),ChrW(57343),2)",
        "StrComp(ChrW(55296),ChrW(57343),-1)",
        "StrComp(ChrW(55296),ChrW(57343))",
        "StrComp(ChrW(55296),ChrW(57343),'2')",
        "StrComp(ChrW(65),'AB',2)",
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Int16(1)), "{sql}");
    }
    assert_eq!(evaluate("StrComp(ChrW(49),1,2)"), Ok(Value::Int16(0)));
}

#[test]
fn adopted_core_string_cells_use_the_same_binary_and_database_paths() {
    for (mode, expected) in [(0, -1), (2, 1)] {
        let mut budget = ResourceBudget::new(64, 0, 1000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Reverse)
            .apply_expression_byte_comparison(
                raw(vec![0, 216]),
                raw(vec![0, 220]),
                Some(ExpressionValue::Value(Value::Int32(mode))),
            );
        assert_eq!(result, Ok(Value::Int16(expected)));
    }
}

#[test]
fn database_comparison_keeps_mode_null_type_and_eager_failure_order() {
    assert_eq!(evaluate("StrComp(ChrB(65),NULL,2)"), Ok(Value::Null));
    for (sql, expected) in [
        ("StrComp(ChrB(65),NULL,3)", EvalError::Domain),
        (
            "StrComp(ChrB(65),NULL,NULL)",
            EvalError::InvalidNullConversion,
        ),
        ("StrComp(ChrB(65),'a',2)", EvalError::UnrepresentableText),
        ("StrComp(ChrW(55296),CVErr(1),2)", EvalError::TypeMismatch),
        ("StrComp(ChrW(55296),1/0,2)", EvalError::DivisionByZero),
        ("StrComp(NULL,ChrW(65536),2)", EvalError::Domain),
        ("StrComp(ChrW(65),'a',1)", EvalError::CollationRequired),
    ] {
        assert_eq!(evaluate(sql), Err(expected), "{sql}");
    }

    let expression = parse_expression("StrComp(ChrW(65),NULL,'2')", Default::default()).unwrap();
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Reverse)
            .evaluate(&expression),
        Err(EvalError::NumberFormatRequired)
    );
}

#[test]
fn database_unit_buffers_and_all_observed_checkpoints_use_the_shared_budget() {
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Reverse)
            .with_cancellation(&observe)
            .apply_expression_byte_comparison(raw(vec![0, 216]), raw(vec![0, 220]), None),
        Ok(Value::Int16(1))
    );
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_collation(&Reverse)
                .with_cancellation(&cancel)
                .apply_expression_byte_comparison(raw(vec![0, 216]), raw(vec![0, 220]), None),
            Err(EvalError::Cancelled)
        );
    }
    let mut budget = ResourceBudget::new(0, 0, 1000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Reverse)
            .apply_expression_byte_comparison(raw(vec![0, 216]), raw(vec![0, 220]), None),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn database_raw_callback_failure_is_never_retried_through_unicode() {
    struct Reject;
    impl Collation for Reject {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            panic!("raw callback failure must propagate");
        }

        fn string_comparison_profile(&self) -> StringComparisonProfile {
            StringComparisonProfile::Utf16
        }

        fn compare_utf16(&self, _: &[u16], _: &[u16]) -> Result<Ordering, EvalError> {
            Err(EvalError::CollationFailed)
        }
    }

    let mut budget = ResourceBudget::new(1000, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&Reject)
            .apply_expression_byte_comparison(
                raw(vec![65, 0]),
                raw(vec![66, 0]),
                Some(ExpressionValue::Value(Value::Int32(2))),
            ),
        Err(EvalError::CollationFailed)
    );
}
