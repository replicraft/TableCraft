use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use crate::functions::conversions::text::NumberFormat;
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_core::values::LegacyDateTime;

#[test]
fn contextual_agreement_reuses_exact_conversion_charges_and_cancellation() {
    for (operator, left, right) in [
        (
            Arithmetic::IntegerDivide,
            Value::Text("7,5".into()),
            Value::Int16(2),
        ),
        (
            Arithmetic::Add,
            Value::DateTime(LegacyDateTime::from_ole_days(2.25).unwrap()),
            Value::Text("0,5".into()),
        ),
    ] {
        let mut budget = ResourceBudget::new(65_536, 0, 65_536);
        let checks = Cell::new(0);
        let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::DeDe)
            .with_cancellation(&|| {
                checks.set(checks.get() + 1);
                false
            })
            .apply_arithmetic_agreement(operator, &left, &right)
            .unwrap()
            .unwrap();
        let total = checks.get();
        let remaining = [
            budget.remaining(Resource::Bytes),
            budget.remaining(Resource::Steps),
        ];

        let mut known_budget = ResourceBudget::new(65_536, 0, 65_536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut known_budget, EvalLimits::default())
            .with_number_format(NumberFormat::DeDe);
        let known = if operator == Arithmetic::IntegerDivide {
            evaluator.apply_declared_rounded_arithmetic(
                operator,
                &left,
                &right,
                ArithmeticDeclarations::default(),
            )
        } else {
            evaluator.apply_declared_numeric_arithmetic(
                operator,
                &left,
                &right,
                ArithmeticDeclarations::default(),
            )
        };
        assert_eq!(known, Ok(result));
        assert_eq!(
            remaining,
            [
                known_budget.remaining(Resource::Bytes),
                known_budget.remaining(Resource::Steps)
            ]
        );
        assert!(remaining[0] < 65_536);
        assert!(total > 0);

        for stop in 1..=total {
            checks.set(0);
            let mut budget = ResourceBudget::new(65_536, 0, 65_536);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_number_format(NumberFormat::DeDe)
                    .with_cancellation(&|| {
                        checks.set(checks.get() + 1);
                        checks.get() >= stop
                    })
                    .apply_arithmetic_agreement(operator, &left, &right),
                Err(EvalError::Cancelled),
                "checkpoint {stop}"
            );
        }

        let mut budget = ResourceBudget::new(0, 0, 65_536);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::DeDe)
                .apply_arithmetic_agreement(operator, &left, &right),
            Err(EvalError::Limit(_))
        ));
    }
}
