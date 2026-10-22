//! Charge stateful math calls before consulting or advancing caller-owned state.
use super::Evaluator;
use crate::{EvalError, functions::math::MathFunction};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Apply evaluated math operands; only Rnd consults the host random sequence.
    /// Earlier successful calls remain consumed if later query work fails.
    pub fn apply_math(
        &mut self,
        function: MathFunction,
        values: &[Value],
    ) -> Result<Value, EvalError> {
        if function == MathFunction::Rnd {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
        }
        function.apply_with_random(values, self.random)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::math::random::RndState,
    };
    use std::cell::Cell;
    use tablecraft_core::limits::ResourceBudget;
    use tablecraft_sql::{parse_select, select::Projection};

    #[test]
    fn cancellation_and_work_refusal_precede_sequence_mutation() {
        for cancelled in [false, true] {
            let state = RndState::default();
            let mut budget = ResourceBudget::new(100, 0, 0);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_random(&state)
                .with_cancellation(&|| cancelled)
                .apply_math(MathFunction::Rnd, &[]);
            if cancelled {
                assert_eq!(result, Err(EvalError::Cancelled));
            } else {
                assert!(matches!(result, Err(EvalError::Limit(_))));
            }
            assert_eq!(state.apply(None), RndState::default().apply(None));
        }
    }

    #[test]
    fn every_scalar_checkpoint_and_invalid_arity_avoid_unearned_draws() {
        let parsed = parse_select("SELECT Rnd(-1)", Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let count = Cell::new(0);
        let mut budget = ResourceBudget::new(100, 0, 100);
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_random(&RndState::default())
            .with_cancellation(&|| {
                count.set(count.get() + 1);
                false
            })
            .evaluate(expression)
            .unwrap();
        let checkpoints = count.get();
        for stop in 1..=checkpoints {
            count.set(0);
            let state = RndState::default();
            let mut budget = ResourceBudget::new(100, 0, 100);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_random(&state)
                    .with_cancellation(&|| {
                        count.set(count.get() + 1);
                        count.get() == stop
                    })
                    .evaluate(expression),
                Err(EvalError::Cancelled)
            );
            assert_eq!(state.apply(None), RndState::default().apply(None));
        }
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .apply_math(MathFunction::Rnd, &[]),
            Err(EvalError::RandomStateRequired)
        );
        let parsed = parse_select("SELECT Rnd(1/0,1)", Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("expression required");
        };
        let state = RndState::default();
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_random(&state)
                .evaluate(expression),
            Err(EvalError::UnsupportedExpression)
        );
        assert_eq!(state.apply(None), RndState::default().apply(None));
    }
}
