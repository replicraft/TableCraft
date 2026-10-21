//! Provider routing controls, not locale-comparison goldens.
use super::*;
use crate::{comparison::LocaleId, functions::text::apply_with_collation};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    ops::Range,
};

struct Host {
    calls: RefCell<Vec<(u32, StringSearch)>>,
    result: Result<Option<Range<usize>>, EvalError>,
    cancelled: Cell<bool>,
    cancel_on_return: bool,
}
impl Host {
    fn new(result: Result<Option<Range<usize>>, EvalError>) -> Self {
        Self {
            calls: RefCell::new(vec![]),
            result,
            cancelled: Cell::new(false),
            cancel_on_return: false,
        }
    }
}
impl Collation for Host {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("LCID became database comparison")
    }
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
    fn find(
        &self,
        _: &str,
        _: &str,
        _: StringSearch,
        _: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        panic!("LCID became module/database search")
    }
    fn find_locale(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!(haystack, "😀Straße");
        assert_eq!(needle, "SS");
        self.calls.borrow_mut().push((locale.raw(), search));
        if self.cancel_on_return {
            self.cancelled.set(true);
        }
        self.result.clone()
    }
}
fn args(mode: Value) -> Vec<Value> {
    vec![
        Value::Int16(3),
        Value::Text("😀Straße".into()),
        Value::Text("SS".into()),
        mode,
    ]
}
fn call(
    host: Option<&dyn Collation>,
    arguments: &[Value],
) -> Result<TextResult<'static>, EvalError> {
    match apply_with_collation(TextFunction::InStr, arguments, host, || Ok(()))? {
        TextResult::Position(value) => Ok(TextResult::Position(value)),
        TextResult::Null => Ok(TextResult::Null),
        _ => unreachable!(),
    }
}
#[test]
fn forward_lcid_preserves_sort_identity_rounding_and_utf16_consumed_bounds() {
    for (value, raw) in [
        (Value::Int32(1033), 1033),
        (Value::Int32(0x10407), 0x10407),
        (
            Value::Double(tablecraft_core::values::Double::from_float(1033.5)),
            1034,
        ),
        (Value::Int32(0x1000), 0x1000),
    ] {
        let host = Host::new(Ok(Some(6..7)));
        assert_eq!(call(Some(&host), &args(value)), Ok(TextResult::Position(7)));
        assert_eq!(*host.calls.borrow(), [(raw, StringSearch::ForwardFrom(2))]);
    }
    let host = Host::new(Ok(None));
    assert_eq!(
        call(Some(&host), &args(Value::Int32(1033))),
        Ok(TextResult::Position(0))
    );
}
#[test]
fn invalid_structures_null_modes_and_unknown_provider_support_stay_distinct() {
    for value in [Value::Int32(-2), Value::Int32(0x100409)] {
        let mut arguments = args(value);
        arguments[1] = Value::Null;
        assert_eq!(call(None, &arguments), Err(EvalError::Domain));
    }
    let mut arguments = args(Value::Null);
    arguments[1] = Value::Null;
    assert_eq!(
        call(None, &arguments),
        Err(EvalError::InvalidNullConversion)
    );
    arguments = args(Value::Int32(1033));
    arguments[1] = Value::Null;
    assert_eq!(call(None, &arguments), Ok(TextResult::Null));
    assert_eq!(
        call(None, &args(Value::Int32(1033))),
        Err(EvalError::CollationRequired)
    );
    // An existing database provider has no implicit locale implementation.
    struct Database;
    impl Collation for Database {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            Ok(Ordering::Equal)
        }
    }
    assert_eq!(
        call(Some(&Database), &args(Value::Int32(1033))),
        Err(EvalError::CollationRequired)
    );
    let host = Host::new(Err(EvalError::CollationRequired));
    assert_eq!(
        call(Some(&host), &args(Value::Int32(0xfffff))),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(host.calls.borrow()[0].0, 0xfffff);
}
#[test]
fn lcid_bounds_errors_and_short_circuits_share_existing_search_rules() {
    for range in [1..3, 2..9, 8..8, Range { start: 7, end: 6 }] {
        let host = Host::new(Ok(Some(range)));
        assert_eq!(
            call(Some(&host), &args(Value::Int32(1033))),
            Err(EvalError::InvalidTextMatch)
        );
    }
    let host = Host::new(Err(EvalError::Cancelled));
    assert_eq!(
        call(Some(&host), &args(Value::Int32(1033))),
        Err(EvalError::Cancelled)
    );
    for (haystack, needle, start, expected) in [("", "x", 1, 0), ("x", "", 1, 1), ("x", "x", 2, 0)]
    {
        let arguments = vec![
            Value::Int32(start),
            Value::Text(haystack.into()),
            Value::Text(needle.into()),
            Value::Int32(1033),
        ];
        assert_eq!(
            call(Some(&host), &arguments),
            Ok(TextResult::Position(expected))
        );
    }
    assert_eq!(host.calls.borrow().len(), 1);
}
#[test]
fn lcid_is_not_silently_enabled_for_other_standard_compare_functions() {
    let host = Host::new(Ok(Some(6..7)));
    for (function, arguments) in [
        (
            TextFunction::StrComp,
            vec![Value::Null, Value::Null, Value::Int32(1033)],
        ),
        (
            TextFunction::InStrRev,
            vec![
                Value::Null,
                Value::Null,
                Value::Int16(-1),
                Value::Int32(1033),
            ],
        ),
        (
            TextFunction::Replace,
            vec![
                Value::Text("x".into()),
                Value::Text("x".into()),
                Value::Text("y".into()),
                Value::Int16(1),
                Value::Int16(-1),
                Value::Int32(1033),
            ],
        ),
    ] {
        assert_eq!(
            apply_with_collation(function, &arguments, Some(&host), || Ok(())),
            Err(EvalError::Domain)
        );
    }
    assert!(host.calls.borrow().is_empty());
}
#[test]
fn lcid_callbacks_preserve_budget_errors_and_check_cancellation_after_return() {
    let host = Host::new(Ok(Some(6..7)));
    let mut calls = 0;
    apply_with_collation(
        TextFunction::InStr,
        &args(Value::Int32(1033)),
        Some(&host),
        || {
            calls += 1;
            Ok(())
        },
    )
    .unwrap();
    for stop in 1..=calls {
        let host = Host::new(Ok(Some(6..7)));
        let mut current = 0;
        assert_eq!(
            apply_with_collation(
                TextFunction::InStr,
                &args(Value::Int32(1033)),
                Some(&host),
                || {
                    current += 1;
                    if current == stop {
                        Err(EvalError::Cancelled)
                    } else {
                        Ok(())
                    }
                }
            ),
            Err(EvalError::Cancelled)
        );
    }
    let mut host = Host::new(Ok(Some(6..7)));
    host.cancel_on_return = true;
    assert_eq!(
        apply_with_collation(
            TextFunction::InStr,
            &args(Value::Int32(1033)),
            Some(&host),
            || if host.cancelled.get() {
                Err(EvalError::Cancelled)
            } else {
                Ok(())
            }
        ),
        Err(EvalError::Cancelled)
    );
    let failure = EvalError::Limit(
        tablecraft_core::limits::ResourceBudget::new(0, 0, 0)
            .charge(tablecraft_core::limits::Resource::Steps, 1)
            .unwrap_err(),
    );
    let host = Host::new(Err(failure));
    assert_eq!(call(Some(&host), &args(Value::Int32(1033))), Err(failure));
}
