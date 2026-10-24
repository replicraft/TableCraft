//! Public evaluator entry points must meter fast paths as well as Text scans.
use super::{Bindings, EvalLimits, Evaluator, NoBindings};
use crate::{
    EvalError,
    functions::{conditional::ConditionalFunction, conversions::Conversion},
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_sql::{
    parse_select,
    select::{FieldReference, Projection},
};

const CONVERSIONS: [Conversion; 9] = [
    Conversion::Boolean,
    Conversion::Byte,
    Conversion::Integer,
    Conversion::Long,
    Conversion::LongLong,
    Conversion::Single,
    Conversion::Double,
    Conversion::Currency,
    Conversion::Decimal,
];

#[test]
fn every_direct_conversion_checks_cancellation_and_work_for_materialized_values() {
    for conversion in CONVERSIONS {
        for value in [Value::Int16(1), Value::Null, Value::Binary(vec![])] {
            let mut budget = ResourceBudget::new(0, 0, 100);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| true)
                    .apply_conversion(conversion, &value),
                Err(EvalError::Cancelled)
            );

            let mut budget = ResourceBudget::new(0, 0, 0);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .apply_conversion(conversion, &value),
                Err(EvalError::Limit(error)) if error.resource == Resource::Steps
            ));

            let mut budget = ResourceBudget::new(0, 0, 1);
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
            assert_eq!(
                evaluator.apply_conversion(conversion, &value),
                conversion.apply(&value)
            );
            assert!(matches!(
                evaluator.apply_conversion(conversion, &value),
                Err(EvalError::Limit(error)) if error.resource == Resource::Steps
            ));
        }
    }
}

#[test]
fn expression_only_conversion_states_do_not_bypass_operational_failures() {
    for conversion in CONVERSIONS {
        for value in [
            ExpressionValue::Error(1),
            ExpressionValue::Error(u32::MAX),
            ExpressionValue::Empty,
            ExpressionValue::Missing,
        ] {
            let mut budget = ResourceBudget::new(0, 0, 100);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| true)
                    .apply_expression_conversion(conversion, &value),
                Err(EvalError::Cancelled)
            );

            let mut budget = ResourceBudget::new(0, 0, 0);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .apply_expression_conversion(conversion, &value),
                Err(EvalError::Limit(error)) if error.resource == Resource::Steps
            ));
        }
    }
}

#[test]
fn direct_selectors_check_non_text_conditions_and_latch_failed_payload_steps() {
    let chosen = ExpressionValue::Value(Value::Int16(7));
    for condition in [
        ExpressionValue::Value(Value::Boolean(true)),
        ExpressionValue::Value(Value::Null),
        ExpressionValue::Error(1),
    ] {
        let mut budget = ResourceBudget::new(0, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_iif(&condition, chosen.clone(), chosen.clone()),
            Err(EvalError::Cancelled)
        );

        let mut budget = ResourceBudget::new(0, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_iif(&condition, chosen.clone(), chosen.clone()),
            Err(EvalError::Limit(error)) if error.resource == Resource::Steps
        ));
    }

    for function in [ConditionalFunction::Choose, ConditionalFunction::Switch] {
        let mut selection = function.start(2).unwrap();
        let mut budget = ResourceBudget::new(0, 0, 1);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        evaluator
            .push_selection_argument(&mut selection, ExpressionValue::Value(Value::Int16(1)))
            .unwrap();
        assert!(matches!(
            evaluator.push_selection_argument(&mut selection, chosen.clone()),
            Err(EvalError::Limit(error)) if error.resource == Resource::Steps
        ));
        assert!(matches!(
            selection.finish_expression(),
            Err(EvalError::Limit(error)) if error.resource == Resource::Steps
        ));
    }
}

struct CancelOnResolve {
    cancelled: Cell<bool>,
    calls: Cell<usize>,
    value: Value,
}

impl Bindings for CancelOnResolve {
    fn resolve(&self, _: &FieldReference) -> Result<&Value, EvalError> {
        self.calls.set(self.calls.get() + 1);
        self.cancelled.set(true);
        Ok(&self.value)
    }
}

#[test]
fn final_eager_selector_callback_cannot_publish_success_after_cancellation() {
    for sql in [
        "SELECT IIf(True,7,Stop)",
        "SELECT Choose(1,7,Stop)",
        "SELECT Switch(True,7,False,Stop)",
    ] {
        let parsed = parse_select(sql, Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let bindings = CancelOnResolve {
            cancelled: Cell::new(false),
            calls: Cell::new(0),
            value: Value::Int16(9),
        };
        let cancelled = || bindings.cancelled.get();
        let mut budget = ResourceBudget::new(1000, 0, 1000);
        assert_eq!(
            Evaluator::new(&bindings, &mut budget, EvalLimits::default())
                .with_cancellation(&cancelled)
                .evaluate_expression_value(expression),
            Err(EvalError::Cancelled),
            "{sql}"
        );
        assert_eq!(bindings.calls.get(), 1);
    }
}
