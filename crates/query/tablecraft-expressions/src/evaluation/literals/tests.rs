use super::*;
use crate::evaluation::{EvalLimits, Evaluator, NoBindings};
use proptest::prelude::*;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::select::Expression;

#[test]
fn radix_literals_select_signed_integer_widths_from_their_magnitudes() {
    for (text, expected) in [
        ("&H0", Value::Int16(0)),
        ("&h7fff", Value::Int16(32767)),
        ("&H8000", Value::Int16(-32768)),
        ("&HFFFF", Value::Int16(-1)),
        ("&H0000FFFF", Value::Int16(-1)),
        ("&H10000", Value::Int32(65536)),
        ("&H7FFFFFFF", Value::Int32(i32::MAX)),
        ("&H80000000", Value::Int32(i32::MIN)),
        ("&HFFFFFFFF", Value::Int32(-1)),
        ("&O77777", Value::Int16(32767)),
        ("&o100000", Value::Int16(-32768)),
        ("&O177777", Value::Int16(-1)),
        ("&O200000", Value::Int32(65536)),
        ("&O37777777777", Value::Int32(-1)),
    ] {
        assert_eq!(number(text), Ok(expected), "{text}");
    }
    assert_eq!(
        number(&format!("&H{}FF", "0".repeat(124))),
        Ok(Value::Int16(255))
    );
    assert_eq!(number("&H100000000"), Err(EvalError::Overflow));
    assert_eq!(number("&O40000000000"), Err(EvalError::Overflow));
}

#[test]
fn malformed_or_oversized_host_spelling_is_rejected_and_node_checks_precede_work() {
    for text in [
        "&H", "&O", "&h+1", "&H-1", "&O8", "&H1%", "&HＦＦ", "&H 1", "&B10", "&77",
    ] {
        assert_eq!(number(text), Err(EvalError::UnsupportedLiteral), "{text}");
    }
    assert_eq!(
        number(&format!("&H{}", "0".repeat(127))),
        Err(EvalError::UnsupportedLiteral)
    );
    let expression = Expression::Number("&HFF".into());
    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(
        matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).evaluate(&expression), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
    );
    let mut budget = ResourceBudget::new(0, 0, 100);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .evaluate(&expression),
        Err(EvalError::Cancelled)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn hexadecimal_and_octal_match_an_independent_signed_magnitude_model(bits in any::<u32>()) {
        let expected = if bits <= 65535 {
            let signed = if bits < 32768 { i64::from(bits) } else { i64::from(bits) - 65536 };
            Value::Int16(i16::try_from(signed).unwrap())
        } else {
            let signed = if bits < 2147483648 { i64::from(bits) } else { i64::from(bits) - 4294967296 };
            Value::Int32(i32::try_from(signed).unwrap())
        };
        prop_assert_eq!(number(&format!("&H{bits:X}")), Ok(expected.clone()));
        prop_assert_eq!(number(&format!("&O{bits:o}")), Ok(expected));
    }
}
