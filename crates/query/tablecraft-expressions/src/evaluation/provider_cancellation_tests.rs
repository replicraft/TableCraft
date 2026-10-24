use super::{EvalLimits, Evaluator, NoBindings};
use crate::{
    EvalError,
    comparison::{Collation, Comparison, LikeProfile, LocaleId, StringCompareMode, StringSearch},
    functions::text::TextFunction,
    logic::Truth,
    predicates::LikeMode,
};
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionValue, Value},
};

#[derive(Default)]
struct CancellingCollation {
    cancelled: Cell<bool>,
    calls: Cell<usize>,
    fail: bool,
    weighted: bool,
}

impl CancellingCollation {
    fn answer<T>(&self, value: T) -> Result<T, EvalError> {
        self.calls.set(self.calls.get() + 1);
        self.cancelled.set(true);
        if self.fail {
            return Err(EvalError::CollationFailed);
        }

        Ok(value)
    }
}

impl Collation for CancellingCollation {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        self.answer(Ordering::Equal)
    }

    fn compare_text(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        self.answer(Ordering::Equal)
    }

    fn like_profile(&self) -> LikeProfile {
        if self.weighted {
            LikeProfile::Utf16Weights
        } else {
            LikeProfile::UnicodeScalar
        }
    }

    fn compare_like_units(&self, _: &[u16], _: &[u16]) -> Result<Ordering, EvalError> {
        self.answer(Ordering::Equal)
    }

    fn is_like_unit(&self, _: &[u16]) -> Result<bool, EvalError> {
        self.answer(true)
    }

    fn find(
        &self,
        _: &str,
        _: &str,
        _: StringSearch,
        _: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.answer(Some(0..1))
    }

    fn find_locale(
        &self,
        _: &str,
        _: &str,
        _: StringSearch,
        _: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.answer(Some(0..1))
    }
}

fn text() -> Value {
    Value::Text("a".into())
}

#[test]
fn direct_predicates_observe_callback_cancellation_and_preserve_provider_errors() {
    for fail in [false, true] {
        for comparison in [
            Comparison::Equal,
            Comparison::NotEqual,
            Comparison::Less,
            Comparison::LessOrEqual,
            Comparison::Greater,
            Comparison::GreaterOrEqual,
        ] {
            let collation = CancellingCollation {
                fail,
                ..Default::default()
            };
            let cancelled = || collation.cancelled.get();
            let mut budget = ResourceBudget::new(65536, 0, 65536);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_collation(&collation)
                .with_cancellation(&cancelled);
            let expected = match fail {
                true => EvalError::CollationFailed,
                false => EvalError::Cancelled,
            };
            assert_eq!(
                evaluator.compare_values(comparison, &text(), &text()),
                Err(expected)
            );
            assert_eq!(collation.calls.get(), 1);
        }
    }
}

#[test]
fn both_strcomp_apis_check_after_text_and_database_callbacks() {
    for mode in [1, 2] {
        for expression_api in [false, true] {
            for fail in [false, true] {
                let collation = CancellingCollation {
                    fail,
                    ..Default::default()
                };
                let cancelled = || collation.cancelled.get();
                let mut budget = ResourceBudget::new(65536, 0, 65536);
                let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_collation(&collation)
                    .with_cancellation(&cancelled);
                let arguments = [text(), text(), Value::Int32(mode)];
                let result = if expression_api {
                    let values = arguments
                        .into_iter()
                        .map(ExpressionValue::Value)
                        .collect::<Vec<_>>();
                    evaluator.apply_expression_comparison(&values)
                } else {
                    evaluator.apply_text_function(TextFunction::StrComp, &arguments)
                };
                let expected = match fail {
                    true => EvalError::CollationFailed,
                    false => EvalError::Cancelled,
                };
                assert_eq!(result, Err(expected));
                assert_eq!(collation.calls.get(), 1);
            }
        }
    }
}

#[test]
fn searches_replacement_and_both_like_profiles_retain_callback_guards() {
    for (function, arguments) in [
        (
            TextFunction::InStr,
            vec![Value::Int32(1), text(), text(), Value::Int32(2)],
        ),
        (
            TextFunction::InStr,
            vec![Value::Int32(1), text(), text(), Value::Int32(1033)],
        ),
        (
            TextFunction::InStrRev,
            vec![text(), text(), Value::Int32(-1), Value::Int32(2)],
        ),
        (
            TextFunction::Replace,
            vec![
                text(),
                text(),
                text(),
                Value::Int32(1),
                Value::Int32(-1),
                Value::Int32(2),
            ],
        ),
    ] {
        let collation = CancellingCollation::default();
        let cancelled = || collation.cancelled.get();
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_collation(&collation)
            .with_cancellation(&cancelled);
        assert_eq!(
            evaluator.apply_text_function(function, &arguments),
            Err(EvalError::Cancelled)
        );
        assert_eq!(collation.calls.get(), 1);
    }

    for weighted in [false, true] {
        for pattern in ["a", "[a]"] {
            let collation = CancellingCollation {
                weighted,
                ..Default::default()
            };
            let cancelled = || collation.cancelled.get();
            let mut budget = ResourceBudget::new(65536, 0, 65536);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_collation(&collation)
                .with_like_mode(LikeMode::Ansi89)
                .with_cancellation(&cancelled);
            assert_eq!(
                evaluator.apply_like(&text(), &Value::Text(pattern.into()), false),
                Err(EvalError::Cancelled)
            );
            assert_eq!(collation.calls.get(), 1);
        }
    }
}

#[test]
fn null_and_numeric_comparisons_do_not_activate_text_providers() {
    let collation = CancellingCollation::default();
    let cancelled = || collation.cancelled.get();
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_collation(&collation)
        .with_cancellation(&cancelled);
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Null, &text()),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Int32(1), &Value::Int32(1)),
        Ok(Truth::True)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::StrComp,
            &[Value::Null, text(), Value::Int32(2)]
        ),
        Ok(Value::Null)
    );
    assert_eq!(collation.calls.get(), 0);
}
