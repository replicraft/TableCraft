//! Count actual provider reads without supplying an implicit civil date.
use std::sync::atomic::{AtomicUsize, Ordering};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};

/// A caller-selected immutable reading with an atomic observation count.
pub struct CountingClock {
    reading: ClockReading,
    calls: AtomicUsize,
}

impl CountingClock {
    /// Retain the actual validated test reading; construction does not read it.
    pub const fn new(reading: ClockReading) -> Self {
        Self {
            reading,
            calls: AtomicUsize::new(0),
        }
    }

    /// Number of actual `Clock::read` callbacks across all shared consumers.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        Ok(self.reading)
    }
}
