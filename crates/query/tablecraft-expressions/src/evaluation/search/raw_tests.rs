use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
    evaluation::{Evaluator, NoBindings},
    functions::{conversions::text::NumberFormat, text::TextFunction},
};
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_sql::parse_expression;

fn evaluate(sql: &str) -> Result<Value, EvalError> {
    let expression = parse_expression(sql, Default::default()).unwrap();
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_number_format(NumberFormat::EnUs)
        .evaluate(&expression)
}

fn raw(units: &[u16]) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
    ))
}

fn integer(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

#[test]
fn binary_search_preserves_complete_units_and_active_unaligned_ranges() {
    for (sql, expected) in [
        ("InStr(1,ChrW(55296)&'a'&ChrW(55296),ChrW(55296),0)", 1),
        ("InStr(2,ChrW(55296)&'a'&ChrW(55296),ChrW(55296),0)", 3),
        ("InStrRev(ChrW(55296)&'a'&ChrW(55296),ChrW(55296))", 3),
        ("InStrRev(ChrW(55296)&'a'&ChrW(55296),ChrW(55296),2,0)", 1),
        ("InStr(1,'😀',ChrW(56832),0)", 2),
        ("InStrRev('😀',ChrW(56832))", 2),
        ("InStrRev('😀',ChrW(56832),1)", 0),
        ("InStr(1,MidB('😀',2,2),ChrW(216),0)", 1),
        ("InStrRev(MidB('😀',2,2),ChrW(216))", 1),
        ("InStr(1,ChrW(0)&'A ',ChrW(0),0)", 1),
        ("InStr(1,ChrW(55296),'',0)", 1),
        ("InStrRev(ChrW(55296),'')", 1),
        ("InStr(2,ChrW(55296),'',0)", 0),
        ("InStr(1,LeftB('',0),ChrW(55296),0)", 0),
        ("InStr(1,ChrW(49)&ChrW(50),12,0)", 1),
        ("InStr('1',ChrW(55296),ChrW(55296),'0')", 1),
    ] {
        assert_eq!(evaluate(sql), Ok(Value::Int32(expected)), "{sql}");
    }
}

#[test]
fn raw_search_keeps_controls_null_states_eagerness_and_provider_boundaries() {
    for (sql, expected) in [
        ("InStr(1,ChrB(65),NULL,0)", Ok(Value::Null)),
        ("InStrRev(NULL,ChrB(65))", Ok(Value::Null)),
        ("InStr(0,ChrW(55296),NULL,0)", Err(EvalError::Domain)),
        ("InStrRev(ChrW(55296),NULL,0)", Err(EvalError::Domain)),
        ("InStr(1,ChrW(55296),NULL,-2)", Err(EvalError::Domain)),
        ("InStrRev(ChrW(55296),NULL,-1,3)", Err(EvalError::Domain)),
        (
            "InStrRev(ChrW(55296),NULL,NULL)",
            Err(EvalError::InvalidNullConversion),
        ),
        (
            "InStr(1,ChrW(55296),NULL,NULL)",
            Err(EvalError::InvalidNullConversion),
        ),
        (
            "InStr(1,ChrB(65),'',0)",
            Err(EvalError::UnrepresentableText),
        ),
        ("InStrRev('',ChrB(65))", Err(EvalError::UnrepresentableText)),
        (
            "InStrRev(ChrW(55296),CVErr(1))",
            Err(EvalError::TypeMismatch),
        ),
        (
            "InStr(1,ChrW(55296),ChrW(55296),1)",
            Err(EvalError::UnrepresentableText),
        ),
        (
            "InStr(1,ChrW(55296),ChrW(55296),1033)",
            Err(EvalError::UnrepresentableText),
        ),
        (
            "InStr(1,ChrW(55296),NULL,1/0)",
            Err(EvalError::DivisionByZero),
        ),
        ("InStrRev(ChrW(55296),1/0)", Err(EvalError::DivisionByZero)),
    ] {
        assert_eq!(evaluate(sql), expected, "{sql}");
    }
}

struct BinaryDefault;

impl Collation for BinaryDefault {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("binary search must not invoke a provider")
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Binary)
    }
}

struct RefusingSearch(Cell<usize>);

impl Collation for RefusingSearch {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("substring search must not use equality comparison")
    }

    fn find(
        &self,
        haystack: &str,
        needle: &str,
        _: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!((haystack, needle), ("A", "a"));
        assert_eq!(mode, StringCompareMode::Database);
        self.0.set(self.0.get() + 1);
        Err(EvalError::CollationFailed)
    }
}

#[test]
fn valid_raw_provider_search_publishes_text_and_never_falls_back_after_failure() {
    let host = RefusingSearch(Cell::new(0));
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .apply_expression_search(
                TextFunction::InStr,
                &[
                    integer(1),
                    raw(&[65]),
                    ExpressionValue::Value(Value::Text("a".into())),
                    integer(2),
                ],
            ),
        Err(EvalError::CollationFailed),
    );
    assert_eq!(host.0.get(), 1);

    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_search(TextFunction::InStrRev, &[raw(&[65]), integer(1)]),
        Err(EvalError::NumberFormatRequired),
    );
}

#[test]
fn borrowed_raw_search_needs_no_payload_allocation_and_honors_binary_defaults() {
    let mut budget = ResourceBudget::new(0, 0, 10000);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, Default::default()).with_collation(&BinaryDefault);
    assert_eq!(
        evaluator.apply_expression_search(TextFunction::InStr, &[raw(&[0xd800, 65]), raw(&[65])]),
        Ok(Value::Int32(2))
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStrRev,
            &[raw(&[0xd800, 65]), raw(&[0xd800]), integer(-1), integer(-1)],
        ),
        Ok(Value::Int32(1))
    );
    assert_eq!(
        evaluator.apply_expression_search(
            TextFunction::InStrRev,
            &[raw(&[0xd800]), ExpressionValue::Empty],
        ),
        Ok(Value::Int32(1))
    );
}

#[test]
fn every_raw_search_scan_checkpoint_and_zero_work_limit_refuse_results() {
    for (function, arguments) in [
        (
            TextFunction::InStr,
            vec![
                integer(1),
                raw(&[0xd800, 65, 65, 66]),
                raw(&[65, 66]),
                integer(0),
            ],
        ),
        (
            TextFunction::InStrRev,
            vec![raw(&[0xd800, 65, 65, 66]), raw(&[65, 66])],
        ),
    ] {
        let calls = Cell::new(0);
        let observe = || {
            calls.set(calls.get() + 1);
            false
        };
        let mut budget = ResourceBudget::new(0, 0, 10000);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_cancellation(&observe)
            .apply_expression_search(function, &arguments)
            .unwrap();

        for stop in 1..=calls.get() {
            let visits = Cell::new(0);
            let cancel = || {
                visits.set(visits.get() + 1);
                visits.get() >= stop
            };
            let mut budget = ResourceBudget::new(0, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_cancellation(&cancel)
                    .apply_expression_search(function, &arguments),
                Err(EvalError::Cancelled),
            );
        }
        let mut budget = ResourceBudget::new(0, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .apply_expression_search(function, &arguments),
            Err(EvalError::Limit(error)) if error.resource == Resource::Steps
        ));
    }
}

proptest! {
    #[test]
    fn raw_search_matches_independent_unit_slice_windows(
        haystack in prop::collection::vec(any::<u16>(), 0..24),
        needle in prop::collection::vec(any::<u16>(), 0..6),
        start in 1usize..28,
        reverse in any::<bool>(),
    ) {
        let expected = if haystack.is_empty() || start > haystack.len() {
            0
        } else if needle.is_empty() {
            start
        } else {
            let matches = haystack
                .windows(needle.len())
                .enumerate()
                .filter(|(index, window)| {
                    *window == needle.as_slice()
                        && if reverse {
                            index + needle.len() <= start
                        } else {
                            index + 1 >= start
                        }
                })
                .map(|(index, _)| index + 1)
                .collect::<Vec<_>>();
            if reverse {
                matches.last().copied().unwrap_or(0)
            } else {
                matches.first().copied().unwrap_or(0)
            }
        };

        let (function, arguments) = if reverse {
            (
                TextFunction::InStrRev,
                vec![raw(&haystack), raw(&needle), integer(start as i32), integer(0)],
            )
        } else {
            (
                TextFunction::InStr,
                vec![integer(start as i32), raw(&haystack), raw(&needle), integer(0)],
            )
        };

        let mut budget = ResourceBudget::new(0, 0, 100000);
        let result = Evaluator::new(&NoBindings, &mut budget, Default::default())
            .apply_expression_search(function, &arguments);
        prop_assert_eq!(result, Ok(Value::Int32(expected as i32)));
    }
}
