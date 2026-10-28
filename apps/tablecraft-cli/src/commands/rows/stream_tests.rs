#[allow(dead_code)]
#[path = "../../../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use super::*;
use std::{
    cell::Cell,
    io::{self, Cursor},
};
use tablecraft_testkit::access as support;

fn new_browser() -> NativeBrowser<'static, Cursor<Vec<u8>>> {
    NativeBrowser::new(
        Cursor::new(fixture::database()),
        native::CATALOG_SCOPE,
        native::budget(),
        &|| false,
    )
}
struct FailingWriter {
    kind: io::ErrorKind,
    flush: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.flush {
            Ok(bytes.len())
        } else {
            Err(io::Error::from(self.kind))
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(self.kind))
    }
}

#[test]
fn output_write_and_flush_errors_stop_cursor_continuation() {
    for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::PermissionDenied] {
        for flush in [false, true] {
            let mut browser = new_browser();
            let mut cursor = browser.cursor("Items").unwrap();
            let result = publish(
                &mut cursor,
                1,
                MAX_STREAM_OUTPUT_BYTES,
                &mut FailingWriter { kind, flush },
            );
            assert!(matches!(result, Err(ExecutionError::Output(error)) if error.kind() == kind));
            assert_eq!(cursor.position(), 1);
        }
    }
}

#[test]
fn cumulative_output_limit_is_checked_before_publishing_each_batch() {
    let mut browser = new_browser();
    let mut cursor = browser.cursor("Items").unwrap();
    let mut output = Vec::new();
    let prefix = b"Id\tScore\n1\t10\n";
    assert!(matches!(
        publish(&mut cursor, 1, prefix.len() as u64, &mut output),
        Err(ExecutionError::Operation(_))
    ));
    assert_eq!(output, prefix);
    let mut browser = new_browser();
    let mut cursor = browser.cursor("Items").unwrap();
    let mut output = Vec::new();
    assert!(publish(&mut cursor, 1, 1, &mut output).is_err());
    assert!(output.is_empty());
}

#[test]
fn cancellation_between_batches_stops_after_the_previous_publication() {
    struct CancelAfterFlush<'a> {
        cancelled: &'a Cell<bool>,
        bytes: Vec<u8>,
    }
    impl Write for CancelAfterFlush<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.cancelled.set(true);
            Ok(())
        }
    }
    let cancelled = Cell::new(false);
    let check = || cancelled.get();
    let mut browser = NativeBrowser::new(
        Cursor::new(fixture::database()),
        native::CATALOG_SCOPE,
        native::budget(),
        &check,
    );
    let mut cursor = browser.cursor("Items").unwrap();
    let mut writer = CancelAfterFlush {
        cancelled: &cancelled,
        bytes: Vec::new(),
    };
    assert!(matches!(
        publish(&mut cursor, 1, MAX_STREAM_OUTPUT_BYTES, &mut writer),
        Err(ExecutionError::Operation(_))
    ));
    assert_eq!(writer.bytes, b"Id\tScore\n1\t10\n");
    assert_eq!(cursor.position(), 1);
    assert!(cursor.is_failed());
}
