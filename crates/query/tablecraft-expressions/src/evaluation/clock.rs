//! Meter host clock callbacks before invocation, independently of SQL traversal.
use super::Evaluator;
use crate::{EvalError, functions::dates::clock::ClockFunction};
use tablecraft_core::{limits::Resource, values::Value};
impl Evaluator<'_> {
    /// Evaluate one zero-argument clock call; callbacks are never retried or cached.
    pub fn apply_clock(&mut self, function: ClockFunction) -> Result<Value, EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        function.apply(self.clock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::dates::clock::{Clock, ClockReading},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Counter(AtomicUsize);
    impl Clock for Counter {
        fn read(&self) -> Result<ClockReading, EvalError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(EvalError::ClockUnavailable)
        }
    }
    #[test]
    fn limits_and_cancellation_precede_clock_callbacks() {
        let clock = Counter(AtomicUsize::new(0));
        let mut budget = tablecraft_core::limits::ResourceBudget::new(0, 0, 0);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_clock(&clock)
                .apply_clock(ClockFunction::Now),
            Err(EvalError::Limit(_))
        ));
        let mut budget = tablecraft_core::limits::ResourceBudget::new(0, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_clock(&clock)
                .with_cancellation(&|| true)
                .apply_clock(ClockFunction::Now),
            Err(EvalError::Cancelled)
        );
        assert_eq!(clock.0.load(Ordering::SeqCst), 0);
    }
}
