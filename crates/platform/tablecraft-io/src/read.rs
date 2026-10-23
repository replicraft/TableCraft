use crate::IoError;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Fill a buffer, checking cancellation and charging a step for each read attempt.
///
/// Interrupted reads retry within the same work allowance. Short reads continue;
/// premature EOF returns `IoError::Io(ErrorKind::UnexpectedEof)`. Completion checks
/// cancellation and charges one more step, including for an empty buffer.
/// Callers must charge requested bytes before calling; this helper only charges
/// steps. A failure may leave a partially filled buffer and advances the source;
/// callers must discard that partial data. Host callbacks own their timeouts.
pub fn read_exact<R: Read + ?Sized>(
    source: &mut R,
    mut remaining: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    while !remaining.is_empty() {
        check(budget, cancelled)?;
        match source.read(remaining) {
            Ok(0) => return Err(IoError::Io(ErrorKind::UnexpectedEof)),
            Ok(count) => {
                remaining = remaining
                    .get_mut(count..)
                    .ok_or(IoError::InvalidReadCount)?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    check(budget, cancelled)
}

/// Seek to an absolute byte offset and fill a buffer using [`read_exact`].
///
/// Checks cancellation and charges a step before seeking. This changes the
/// source's cursor; it is not an OS positional read or a concurrent-file contract.
/// A reported position mismatch fails before any read. Byte charging, partial
/// data and callback timeout rules match [`read_exact`].
pub fn read_exact_at<R: Read + Seek + ?Sized>(
    source: &mut R,
    offset: u64,
    buffer: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    let _span = tracing::trace_span!("io.read_at", bytes = buffer.len()).entered();
    check(budget, cancelled)?;
    if source.seek(SeekFrom::Start(offset))? != offset {
        return Err(IoError::InvalidSeekPosition);
    }
    read_exact(source, buffer, budget, cancelled)
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), IoError> {
    if cancelled() {
        return Err(IoError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        io::{self, Cursor},
    };

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
        assert!(matches!(error, IoError::Limit(_)));
        assert_eq!(calls.get(), 5);
        calls.set(0);
        assert_eq!(
            read_exact(
                &mut source,
                &mut bytes,
                &mut ResourceBudget::new(100, 10, 100),
                &|| calls.get() >= 3
            ),
            Err(IoError::Cancelled)
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
            Err(IoError::Cancelled)
        );
    }

    struct ShortReader {
        cursor: Cursor<Vec<u8>>,
        interrupt: bool,
    }
    impl Read for ShortReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if std::mem::take(&mut self.interrupt) {
                return Err(ErrorKind::Interrupted.into());
            }
            let length = buffer.len().min(2);
            self.cursor.read(&mut buffer[..length])
        }
    }
    impl Seek for ShortReader {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.cursor.seek(position)
        }
    }
    #[test]
    fn absolute_reads_fill_short_chunks_without_double_charging_bytes() {
        let mut source = ShortReader {
            cursor: Cursor::new(b"___abcdef!".to_vec()),
            interrupt: true,
        };
        let mut budget = ResourceBudget::new(20, 0, 10);
        budget.charge(Resource::Bytes, 6).unwrap();
        let mut buffer = [0; 6];
        read_exact_at(&mut source, 3, &mut buffer, &mut budget, &|| false).unwrap();
        assert_eq!(&buffer, b"abcdef");
        assert_eq!(source.cursor.position(), 9);
        assert_eq!(budget.remaining(Resource::Bytes), 14);
        assert_eq!(budget.remaining(Resource::Steps), 4);
    }
    #[test]
    fn eof_and_host_errors_remain_typed_and_omit_messages() {
        let mut buffer = [0; 4];
        assert_eq!(
            read_exact(
                &mut b"abc".as_slice(),
                &mut buffer,
                &mut ResourceBudget::new(0, 0, 10),
                &|| false
            ),
            Err(IoError::Io(ErrorKind::UnexpectedEof))
        );
        assert_eq!(&buffer[..3], b"abc");
        struct Fails;
        impl Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "private host payload",
                ))
            }
        }
        let error = read_exact(
            &mut Fails,
            &mut buffer,
            &mut ResourceBudget::new(0, 0, 10),
            &|| false,
        )
        .unwrap_err();
        assert_eq!(error, IoError::Io(ErrorKind::PermissionDenied));
        assert!(!error.to_string().contains("private"));
    }
    #[test]
    fn invalid_reader_counts_return_an_error_without_panicking() {
        struct Invalid;
        impl Read for Invalid {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                Ok(buffer.len() + 1)
            }
        }
        assert_eq!(
            read_exact(
                &mut Invalid,
                &mut [0; 4],
                &mut ResourceBudget::new(0, 0, 10),
                &|| false
            ),
            Err(IoError::InvalidReadCount)
        );
    }
    #[test]
    fn cancellation_and_exhaustion_stop_before_seek_and_seek_errors_are_sanitized() {
        struct SeekFails;
        impl Read for SeekFails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("must not read")
            }
        }
        impl Seek for SeekFails {
            fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
                Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    "private seek payload",
                ))
            }
        }
        assert_eq!(
            read_exact_at(
                &mut SeekFails,
                7,
                &mut [0; 4],
                &mut ResourceBudget::new(0, 0, 10),
                &|| true
            ),
            Err(IoError::Cancelled)
        );
        assert!(matches!(
            read_exact_at(
                &mut SeekFails,
                7,
                &mut [0; 4],
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            ),
            Err(IoError::Limit(_))
        ));
        let error = read_exact_at(
            &mut SeekFails,
            7,
            &mut [0; 4],
            &mut ResourceBudget::new(0, 0, 10),
            &|| false,
        )
        .unwrap_err();
        assert_eq!(error, IoError::Io(ErrorKind::PermissionDenied));
        assert!(!error.to_string().contains("private"));
    }
}
