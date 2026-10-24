//! Continuation failures must never turn a consumed suffix into a valid table.
use super::*;
use std::{
    cell::RefCell,
    io::{self, Read, Seek, SeekFrom},
};
use tablecraft_ace::{NativeReadSource, TableCursor, TableSnapshotLimits};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Read,
    Seek,
    Decode,
}

#[derive(Clone, Copy, Debug)]
enum Failure {
    Io,
    Eof,
    InvalidCount,
    WrongPosition,
    Decode,
    Cancel,
}

#[derive(Default)]
struct Controls {
    operations: RefCell<Vec<Operation>>,
    fail_at: Cell<Option<usize>>,
    cancelled: Cell<bool>,
}

impl Controls {
    fn call(&self, operation: Operation) -> bool {
        let mut operations = self.operations.borrow_mut();
        operations.push(operation);
        self.fail_at.get() == Some(operations.len())
    }

    fn count(&self) -> usize {
        self.operations.borrow().len()
    }
}

struct FaultSource<'a> {
    bytes: Cursor<&'a [u8]>,
    controls: &'a Controls,
    failure: Failure,
}

impl Read for FaultSource<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.controls.call(Operation::Read) {
            return match self.failure {
                Failure::Eof => Ok(0),
                Failure::InvalidCount => Ok(buffer.len() + 1),
                _ => Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "private details",
                )),
            };
        }
        self.bytes.read(buffer)
    }
}

impl Seek for FaultSource<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.controls.call(Operation::Seek) {
            if let (Failure::WrongPosition, SeekFrom::Start(offset)) = (self.failure, position) {
                return Ok(offset ^ 1);
            }
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private details",
            ));
        }
        self.bytes.seek(position)
    }
}

impl NativeReadSource for FaultSource<'_> {
    fn decode_native_page(
        &mut self,
        _: u32,
        page: &mut [u8],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if self.controls.call(Operation::Decode) {
            // Even a hook that modifies its local buffer must not leak a batch.
            page.fill(0);
            if matches!(self.failure, Failure::Cancel) {
                self.controls.cancelled.set(true);
                return Ok(());
            }
            return Err(InspectError::UnsupportedEncryption);
        }
        Ok(())
    }
}

fn profile(count: usize, code: u8) -> Vec<u8> {
    let mut bytes = database(count);
    bytes[20] = code;
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes
}

#[test]
fn every_continuation_io_and_page_hook_failure_closes_the_cursor_without_a_partial_batch() {
    for code in [1, 2, 5, 6] {
        let bytes = profile(403, code);
        let controls = Controls::default();
        let mut source = FaultSource {
            bytes: Cursor::new(bytes.as_slice()),
            controls: &controls,
            failure: Failure::Io,
        };
        let prefix_end;
        {
            let mut cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut budget(), &|| false).unwrap();
            let prefix = cursor.next_page(1, &mut budget(), &|| false).unwrap();
            assert_eq!(prefix.rows(), expected(0, 1));
            prefix_end = controls.count();
            let suffix = cursor.next_page(500, &mut budget(), &|| false).unwrap();
            assert_eq!(suffix.rows(), expected(1, 402));
            assert!(suffix.reached_end());
        }
        let operations = controls.operations.into_inner();
        assert!(
            operations[prefix_end..]
                .iter()
                .any(|op| matches!(op, Operation::Decode))
        );

        for (index, operation) in operations.iter().enumerate().skip(prefix_end) {
            let failures: &[Failure] = match operation {
                Operation::Read => &[Failure::Io, Failure::Eof, Failure::InvalidCount],
                Operation::Seek => &[Failure::Io, Failure::WrongPosition],
                Operation::Decode => &[Failure::Decode, Failure::Cancel],
            };
            for &failure in failures {
                let controls = Controls::default();
                controls.fail_at.set(Some(index + 1));
                let mut source = FaultSource {
                    bytes: Cursor::new(bytes.as_slice()),
                    controls: &controls,
                    failure,
                };
                let mut allowance = budget();
                let mut cursor =
                    TableCursor::open(&mut source, SCOPE, "Items", &mut allowance, &|| {
                        controls.cancelled.get()
                    })
                    .unwrap();
                cursor
                    .next_page(1, &mut allowance, &|| controls.cancelled.get())
                    .unwrap();
                let before = allowance.remaining(Resource::Steps);
                let error = cursor
                    .next_page(500, &mut allowance, &|| controls.cancelled.get())
                    .err()
                    .expect("the entire suffix must be discarded");
                let expected_error = match failure {
                    Failure::Io => InspectError::Io(io::ErrorKind::PermissionDenied),
                    Failure::Eof => InspectError::Truncated,
                    Failure::InvalidCount | Failure::WrongPosition => InspectError::Malformed,
                    Failure::Decode => InspectError::UnsupportedEncryption,
                    Failure::Cancel => InspectError::Cancelled,
                };
                assert_eq!(
                    error, expected_error,
                    "code={code}, callback={index}, {failure:?}"
                );
                assert_eq!(controls.count(), index + 1);
                assert!(!error.to_string().contains("private"));
                assert!(allowance.remaining(Resource::Steps) < before);
                assert_eq!(cursor.position(), 1);
                assert!(cursor.is_failed());

                let callbacks = controls.count();
                let work = allowance.remaining(Resource::Steps);
                assert_eq!(
                    cursor.next_page(1, &mut allowance, &|| false).err(),
                    Some(InspectError::CursorFailed)
                );
                assert_eq!(controls.count(), callbacks);
                assert_eq!(allowance.remaining(Resource::Steps), work);
                assert_eq!(
                    cursor
                        .into_snapshot(TableSnapshotLimits::default(), &mut allowance, &|| false)
                        .err(),
                    Some(InspectError::CursorFailed)
                );
                assert_eq!(controls.count(), callbacks);
                assert_eq!(allowance.remaining(Resource::Steps), work);
                assert_eq!(source.bytes.get_ref(), &bytes.as_slice());
            }
        }
    }
}

struct ShortSource<'a> {
    bytes: Cursor<&'a [u8]>,
    chunk: usize,
    reads: usize,
    stalled: &'a Cell<bool>,
}

impl Read for ShortSource<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        if self.stalled.get() || self.reads.is_multiple_of(7) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = buffer.len().min(self.chunk);
        self.bytes.read(&mut buffer[..count])
    }
}

impl Seek for ShortSource<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl NativeReadSource for ShortSource<'_> {}

#[test]
fn short_interrupted_reads_retain_complete_continuation_without_resetting_work() {
    for code in [1, 2, 5, 6] {
        let bytes = profile(403, code);
        for chunk in [1, 17, 4095] {
            let stalled = Cell::new(false);
            let mut source = ShortSource {
                bytes: Cursor::new(bytes.as_slice()),
                chunk,
                reads: 0,
                stalled: &stalled,
            };
            let mut allowance = budget();
            let initial_work = allowance.remaining(Resource::Steps);
            let mut cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut allowance, &|| false).unwrap();
            let mut offset = 0;
            loop {
                let batch = cursor.next_page(127, &mut allowance, &|| false).unwrap();
                assert_eq!(batch.offset(), offset as u32);
                assert_eq!(batch.rows(), expected(offset, (403 - offset).min(127)));
                offset += batch.rows().len();
                if batch.reached_end() {
                    break;
                }
            }
            assert_eq!(cursor.position(), 403);
            assert!(!cursor.is_failed());
            assert!(allowance.remaining(Resource::Steps) < initial_work);
            assert_eq!(source.bytes.get_ref(), &bytes.as_slice());
        }
    }
}

#[test]
fn persistent_interruption_exhausts_shared_work_and_never_retries_a_failed_cursor() {
    let bytes = profile(403, 5);
    let stalled = Cell::new(false);
    let mut source = ShortSource {
        bytes: Cursor::new(bytes.as_slice()),
        chunk: 17,
        reads: 0,
        stalled: &stalled,
    };
    let mut allowance = budget();
    let mut cursor =
        TableCursor::open(&mut source, SCOPE, "Items", &mut allowance, &|| false).unwrap();
    cursor.next_page(1, &mut allowance, &|| false).unwrap();
    stalled.set(true);

    // Enough to consume buffered rows and reach the next physical read, then
    // bound its repeated Interrupted callbacks under this same explicit pool.
    let mut remaining = ResourceBudget::new(16 * 1024 * 1024, 16_384, 10_000);
    assert!(matches!(
        cursor.next_page(500, &mut remaining, &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(remaining.remaining(Resource::Steps), 0);
    assert_eq!(cursor.position(), 1);
    assert!(cursor.is_failed());
    assert_eq!(
        cursor.next_page(1, &mut allowance, &|| false).err(),
        Some(InspectError::CursorFailed)
    );
    assert_eq!(source.bytes.get_ref(), &bytes.as_slice());
}
