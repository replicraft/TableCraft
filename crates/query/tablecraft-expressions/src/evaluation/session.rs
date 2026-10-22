//! Meter session-name materialization through the ordinary Text boundary.
use super::Evaluator;
use crate::EvalError;
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Return the exact caller-issued account name, checking cancellation, work
    /// and Text allocation limits. No OS/database/security provider is consulted.
    pub fn apply_current_user(&mut self) -> Result<Value, EvalError> {
        self.check(0)?;
        let session = self
            .access_session
            .ok_or(EvalError::AccessSessionRequired)?;
        self.text(session.current_user())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::session::AccessSessionContext,
    };
    use std::cell::Cell;
    use tablecraft_core::limits::ResourceBudget;
    use tablecraft_sql::{parse_select, select::Projection};

    #[test]
    fn current_user_requires_context_and_charges_exact_output_bytes() {
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default()).apply_current_user(),
            Err(EvalError::AccessSessionRequired)
        );
        let session = AccessSessionContext::new("é中🦀").unwrap();
        let mut budget = ResourceBudget::new(9, 0, 1);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_access_session(session)
                .apply_current_user(),
            Ok(Value::Text("é中🦀".into()))
        );
        for (bytes, steps) in [(8, 1), (9, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            assert!(matches!(
                Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_access_session(session)
                    .apply_current_user(),
                Err(EvalError::Limit(_))
            ));
        }
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 8,
                    ..Default::default()
                }
            )
            .with_access_session(session)
            .apply_current_user(),
            Err(EvalError::TextLimit)
        );
    }

    #[test]
    fn scalar_and_dynamic_calls_share_session_and_all_cancellation_checkpoints() {
        let parsed = parse_select("SELECT Eval('CurrentUser()')", Default::default()).unwrap();
        let Projection::Expression { expression, .. } = &parsed.query().projection[0] else {
            panic!("scalar expression required");
        };
        let session = AccessSessionContext::new("Reader").unwrap();
        let observed = Cell::new(0);
        let count = || {
            observed.set(observed.get() + 1);
            false
        };
        let mut budget = ResourceBudget::new(100_000, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_access_session(session)
                .with_cancellation(&count)
                .evaluate(expression),
            Ok(Value::Text("Reader".into()))
        );
        for stop in 1..=observed.get() {
            let checks = Cell::new(0);
            let cancelled = || {
                checks.set(checks.get() + 1);
                checks.get() >= stop
            };
            let mut budget = ResourceBudget::new(100_000, 0, 10000);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, Default::default())
                    .with_access_session(session)
                    .with_cancellation(&cancelled)
                    .evaluate(expression),
                Err(EvalError::Cancelled)
            );
        }
    }
}
