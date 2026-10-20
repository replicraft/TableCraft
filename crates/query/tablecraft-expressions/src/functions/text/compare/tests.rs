use super::*;
use crate::functions::text::{TextFunction, apply, apply_with_collation};
use proptest::prelude::*;
use std::cell::Cell;
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn binary(a: &str, b: &str) -> Result<TextResult<'static>, EvalError> {
    let values = [text(a), text(b), Value::Int16(0)];
    Ok(match apply(TextFunction::StrComp, &values, || Ok(()))? {
        TextResult::Null => TextResult::Null,
        TextResult::Comparison(v) => TextResult::Comparison(v),
        _ => unreachable!(),
    })
}
#[test]
fn binary_compares_exact_utf16_units_with_nuls_spaces_and_no_normalization() {
    for (a, b, result) in [
        ("ABCD", "abcd", -1),
        ("abcd", "ABCD", 1),
        ("", "", 0),
        ("", "a", -1),
        ("a", "a ", -1),
        ("a\0x", "a\0y", -1),
        ("😀", "\u{e000}", -1),
        ("é", "e\u{301}", 1),
    ] {
        assert_eq!(
            binary(a, b),
            Ok(TextResult::Comparison(result)),
            "{a:?} {b:?}"
        );
    }
}
struct Host {
    option: Option<StringCompareMode>,
    database: Cell<usize>,
    text: Cell<usize>,
}
impl Collation for Host {
    fn option_compare(&self) -> Option<StringCompareMode> {
        self.option
    }
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.database.set(self.database.get() + 1);
        Ok(left.cmp(right))
    }
    fn compare_text(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.text.set(self.text.get() + 1);
        Ok(left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()))
    }
}
#[test]
fn default_text_and_database_modes_select_separate_explicit_host_contracts() {
    let host = Host {
        option: Some(StringCompareMode::Text),
        database: Cell::new(0),
        text: Cell::new(0),
    };
    for (mode, expected) in [
        (None, 0),
        (Some(-1), 0),
        (Some(0), -1),
        (Some(1), 0),
        (Some(2), -1),
    ] {
        let mut args = vec![text("ABCD"), text("abcd")];
        if let Some(mode) = mode {
            args.push(Value::Int16(mode));
        }
        assert_eq!(
            apply_with_collation(TextFunction::StrComp, &args, Some(&host), || Ok(())),
            Ok(TextResult::Comparison(expected))
        );
    }
    assert_eq!(host.database.get(), 1);
    assert_eq!(host.text.get(), 3);
    let host = Host {
        option: None,
        database: Cell::new(0),
        text: Cell::new(0),
    };
    assert_eq!(
        apply_with_collation(
            TextFunction::StrComp,
            &[text("a"), text("a")],
            Some(&host),
            || Ok(())
        ),
        Err(EvalError::CollationRequired)
    );
}
#[test]
fn compare_arguments_are_checked_before_null_propagation_without_coercing_strings() {
    for mode in [
        Value::Null,
        Value::Int16(3),
        Value::Int16(-2),
        Value::Int64(i64::MAX),
    ] {
        assert!(
            apply(
                TextFunction::StrComp,
                &[Value::Null, Value::Null, mode],
                || Ok(())
            )
            .is_err()
        );
    }
    assert_eq!(
        apply(
            TextFunction::StrComp,
            &[Value::Null, text("a"), Value::Null],
            || Ok(())
        ),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        apply(
            TextFunction::StrComp,
            &[Value::Null, text("a"), Value::Int16(1)],
            || Ok(())
        ),
        Ok(TextResult::Null)
    );
    assert_eq!(
        apply(
            TextFunction::StrComp,
            &[Value::Int16(1), text("a"), Value::Int16(0)],
            || Ok(())
        ),
        Err(EvalError::TypeMismatch)
    );
    for args in [
        vec![text("a")],
        vec![text("a"), text("b"), Value::Int16(0), Value::Int16(0)],
    ] {
        assert_eq!(
            apply(TextFunction::StrComp, &args, || Ok(())),
            Err(EvalError::UnsupportedExpression)
        );
    }
}
#[test]
fn metered_host_scans_stop_before_callbacks_when_a_step_fails() {
    let host = Host {
        option: Some(StringCompareMode::Text),
        database: Cell::new(0),
        text: Cell::new(0),
    };
    let mut left = 2;
    assert_eq!(
        apply_with_collation(
            TextFunction::StrComp,
            &[text("abcd"), text("abcd"), Value::Int16(1)],
            Some(&host),
            || {
                if left == 0 {
                    return Err(EvalError::Cancelled);
                }
                left -= 1;
                Ok(())
            }
        ),
        Err(EvalError::Cancelled)
    );
    assert_eq!(host.text.get(), 0);
}
proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn binary_order_matches_independent_big_endian_utf16_byte_buffers(a in ".{0,32}",b in ".{0,32}") {
        let bytes=|s:&str|s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<_>>();
        let result=match bytes(&a).cmp(&bytes(&b)){Ordering::Less=>-1,Ordering::Equal=>0,Ordering::Greater=>1};
        prop_assert_eq!(binary(&a,&b),Ok(TextResult::Comparison(result)));
    }
}
