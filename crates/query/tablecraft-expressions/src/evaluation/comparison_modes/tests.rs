use super::*;
use crate::{
    comparison::{Collation, LocaleId, StringSearch},
    evaluation::{EvalLimits, NoBindings},
    functions::{conversions::text::NumberFormat, text::TextFunction},
};
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::limits::ResourceBudget;

struct LocaleHost(Cell<u32>);
impl Collation for LocaleHost {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    fn find_locale(
        &self,
        _: &str,
        _: &str,
        search: StringSearch,
        locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!(search, StringSearch::ForwardFrom(0));
        self.0.set(locale.raw());
        Ok(Some(1..2))
    }
}

#[test]
fn text_modes_preserve_lcid_dispatch_and_closed_mode_refusals() {
    let host = LocaleHost(Cell::new(0));
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_collation(&host);
    let arguments = [
        Value::Int16(1),
        Value::Text("abc".into()),
        Value::Text("b".into()),
        Value::Text("1033".into()),
    ];
    assert_eq!(
        evaluator.apply_text_function(TextFunction::InStr, &arguments),
        Ok(Value::Int32(2))
    );
    assert_eq!(host.0.get(), 1033);
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::InStrRev,
            &[
                Value::Text("abc".into()),
                Value::Text("b".into()),
                Value::Int16(-1),
                Value::Text("1033".into())
            ]
        ),
        Err(EvalError::Domain)
    );
    assert_eq!(
        evaluator.apply_text_function(
            TextFunction::StrComp,
            &[
                Value::Null,
                Value::Text("x".into()),
                Value::Text("bad".into())
            ]
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn text_mode_normalization_borrows_operands_and_checks_every_checkpoint() {
    let arguments = [
        Value::Text("abcdef".into()),
        Value::Text("abcdef".into()),
        Value::Text("0".into()),
    ];
    let mut budget = ResourceBudget::new(1, 0, 1000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::StrComp, &arguments),
        Ok(Value::Int16(0))
    );
    let observed = Cell::new(0);
    let callback = || {
        observed.set(observed.get() + 1);
        false
    };
    let mut budget = ResourceBudget::new(1000, 0, 1000);
    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs)
        .with_cancellation(&callback)
        .apply_text_function(TextFunction::StrComp, &arguments)
        .unwrap();
    for stop in 1..=observed.get() {
        let current = Cell::new(0);
        let callback = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .with_cancellation(&callback)
                .apply_text_function(TextFunction::StrComp, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}
