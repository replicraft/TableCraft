use crate::{InspectError, header::check};
use std::io::{ErrorKind, Read};
use tablecraft_core::limits::ResourceBudget;

// Callers charge the requested bytes once, before any I/O. Each read attempt,
// including an Interrupted retry, consumes work and checks cancellation. The
// host remains responsible for timeouts within an individual blocking callback.
pub(crate) fn read_exact<R: Read>(
    source: &mut R,
    mut remaining: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    while !remaining.is_empty() {
        check(budget, cancelled)?;
        match source.read(remaining) {
            Ok(0) => return Err(InspectError::Truncated),
            Ok(count) => {
                remaining = remaining.get_mut(count..).ok_or(InspectError::Malformed)?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    check(budget, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Interrupts<'a>(&'a Cell<usize>);
    impl Read for Interrupts<'_> {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            self.0.set(self.0.get() + 1);
            Err(ErrorKind::Interrupted.into())
        }
    }
    #[test]
    fn interrupted_reads_consume_steps_and_observe_cancellation() {
        let calls = Cell::new(0);
        let mut bytes = [0; 8];
        let mut source = Interrupts(&calls);
        let error = read_exact(
            &mut source,
            &mut bytes,
            &mut ResourceBudget::new(100, 10, 5),
            &|| false,
        )
        .unwrap_err();
        assert!(matches!(error, InspectError::Limit(_)));
        assert_eq!(calls.get(), 5);
        calls.set(0);
        assert_eq!(
            read_exact(
                &mut source,
                &mut bytes,
                &mut ResourceBudget::new(100, 10, 100),
                &|| calls.get() >= 3
            ),
            Err(InspectError::Cancelled)
        );
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn cancellation_during_final_read_is_observed_before_success() {
        struct Cancels<'a>(&'a Cell<bool>);
        impl Read for Cancels<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                bytes.fill(42);
                self.0.set(true);
                Ok(bytes.len())
            }
        }
        let cancelled = Cell::new(false);
        assert_eq!(
            read_exact(
                &mut Cancels(&cancelled),
                &mut [0; 8],
                &mut ResourceBudget::new(100, 10, 100),
                &|| cancelled.get()
            ),
            Err(InspectError::Cancelled)
        );
    }
}
