use super::*;
use crate::{read_exact_at, write_exact_at};
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Write},
};
use tablecraft_core::limits::Resource;

#[derive(Clone, Copy)]
enum Mode {
    Interrupts(usize),
    Stalled,
    Error(ErrorKind),
    WrongPosition,
    Cancel,
}

struct Source<'a> {
    bytes: Cursor<Vec<u8>>,
    mode: Mode,
    attempts: Vec<SeekFrom>,
    reads: usize,
    writes: usize,
    cancelled: &'a Cell<bool>,
}

impl<'a> Source<'a> {
    fn new(mode: Mode, cancelled: &'a Cell<bool>) -> Self {
        Self {
            bytes: Cursor::new(b"0123456789".to_vec()),
            mode,
            attempts: Vec::new(),
            reads: 0,
            writes: 0,
            cancelled,
        }
    }
}

impl Seek for Source<'_> {
    fn seek(&mut self, target: SeekFrom) -> io::Result<u64> {
        self.attempts.push(target);
        match self.mode {
            Mode::Interrupts(left) if left != 0 => {
                self.mode = Mode::Interrupts(left - 1);
                // A failed attempt need not preserve cursor position. Absolute
                // retries must still reach the original target.
                self.bytes.set_position(1);
                return Err(ErrorKind::Interrupted.into());
            }
            Mode::Stalled => return Err(ErrorKind::Interrupted.into()),
            Mode::Error(kind) => return Err(io::Error::new(kind, "private host details")),
            Mode::WrongPosition => return Ok(5),
            Mode::Cancel => self.cancelled.set(true),
            Mode::Interrupts(_) => {}
        }
        self.bytes.seek(target)
    }
}

impl Read for Source<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.bytes.read(buffer)
    }
}

impl Write for Source<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("bounded seek/write does not flush")
    }
}

#[test]
fn interrupted_absolute_and_end_targets_replay_even_after_cursor_movement() {
    let cancelled = Cell::new(false);
    let mut source = Source::new(Mode::Interrupts(2), &cancelled);
    let mut budget = ResourceBudget::new(100, 0, 10);
    seek_absolute(&mut source, 4, &mut budget, &|| cancelled.get()).unwrap();
    assert_eq!(source.attempts, [SeekFrom::Start(4); 3]);
    assert_eq!(source.bytes.position(), 4);
    assert_eq!(budget.remaining(Resource::Steps), 6);
    assert_eq!(budget.remaining(Resource::Bytes), 100);

    source.mode = Mode::Interrupts(1);
    source.attempts.clear();
    assert_eq!(seek_to_end(&mut source, &mut budget, &|| false), Ok(10));
    assert_eq!(source.attempts, [SeekFrom::End(0); 2]);
    assert_eq!(source.bytes.position(), 10);
    assert_eq!(budget.remaining(Resource::Steps), 3);
    assert_eq!(source.bytes.get_ref(), b"0123456789");
}

#[test]
fn persistent_interrupts_and_cancellation_never_reset_the_work_allowance() {
    let cancelled = Cell::new(false);
    for end in [false, true] {
        let mut source = Source::new(Mode::Stalled, &cancelled);
        let mut budget = ResourceBudget::new(100, 0, 5);
        let result = if end {
            seek_to_end(&mut source, &mut budget, &|| false).map(|_| ())
        } else {
            seek_absolute(&mut source, 4, &mut budget, &|| false)
        };
        assert!(matches!(result, Err(IoError::Limit(_))));
        assert_eq!(source.attempts.len(), 5);
        assert_eq!(budget.remaining(Resource::Steps), 0);
        assert_eq!(budget.remaining(Resource::Bytes), 100);
    }

    let checks = Cell::new(0);
    let mut source = Source::new(Mode::Stalled, &cancelled);
    let mut budget = ResourceBudget::new(100, 0, 20);
    assert_eq!(
        seek_absolute(&mut source, 4, &mut budget, &|| {
            checks.set(checks.get() + 1);
            checks.get() > 3
        }),
        Err(IoError::Cancelled)
    );
    assert_eq!(source.attempts.len(), 3);
    assert_eq!(budget.remaining(Resource::Steps), 17);
}

#[test]
fn other_failures_and_wrong_positions_do_not_retry_or_start_data_io() {
    let cancelled = Cell::new(false);
    for (mode, expected) in [
        (
            Mode::Error(ErrorKind::PermissionDenied),
            IoError::Io(ErrorKind::PermissionDenied),
        ),
        (
            Mode::Error(ErrorKind::WouldBlock),
            IoError::Io(ErrorKind::WouldBlock),
        ),
        (Mode::WrongPosition, IoError::InvalidSeekPosition),
    ] {
        for write in [false, true] {
            let mut source = Source::new(mode, &cancelled);
            let mut budget = ResourceBudget::new(100, 0, 20);
            let result = if write {
                write_exact_at(&mut source, 4, b"xy", &mut budget, &|| false)
            } else {
                read_exact_at(&mut source, 4, &mut [0; 2], &mut budget, &|| false)
            };
            assert_eq!(result.as_ref(), Err(&expected));
            assert_eq!(source.attempts.len(), 1);
            assert_eq!((source.reads, source.writes), (0, 0));
            assert_eq!(budget.remaining(Resource::Steps), 19);
            assert!(!expected.to_string().contains("private"));
        }
    }
}

#[test]
fn cancellation_during_a_successful_seek_prevents_reads_and_writes_including_empty_buffers() {
    let cancelled = Cell::new(false);
    for write in [false, true] {
        for empty in [false, true] {
            cancelled.set(false);
            let mut source = Source::new(Mode::Cancel, &cancelled);
            let mut budget = ResourceBudget::new(100, 0, 20);
            let mut buffer = [0; 2];
            let length = if empty { 0 } else { buffer.len() };
            let result = if write {
                write_exact_at(&mut source, 4, &b"xy"[..length], &mut budget, &|| {
                    cancelled.get()
                })
            } else {
                read_exact_at(&mut source, 4, &mut buffer[..length], &mut budget, &|| {
                    cancelled.get()
                })
            };
            assert_eq!(result, Err(IoError::Cancelled));
            assert_eq!(source.bytes.position(), 4);
            assert_eq!((source.reads, source.writes), (0, 0));
            assert_eq!(source.bytes.get_ref(), b"0123456789");
        }
    }

    cancelled.set(false);
    let mut source = Source::new(Mode::Cancel, &cancelled);
    assert_eq!(
        seek_to_end(&mut source, &mut ResourceBudget::new(100, 0, 20), &|| {
            cancelled.get()
        }),
        Err(IoError::Cancelled)
    );
}

#[test]
fn integrated_reads_and_writes_reach_the_original_absolute_target_after_interruptions() {
    let cancelled = Cell::new(false);
    let mut source = Source::new(Mode::Interrupts(2), &cancelled);
    let mut buffer = [0; 2];
    read_exact_at(
        &mut source,
        4,
        &mut buffer,
        &mut ResourceBudget::new(0, 0, 20),
        &|| false,
    )
    .unwrap();
    assert_eq!(&buffer, b"45");
    assert_eq!((source.reads, source.writes), (1, 0));

    source.mode = Mode::Interrupts(2);
    write_exact_at(
        &mut source,
        4,
        b"xy",
        &mut ResourceBudget::new(0, 0, 20),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.bytes.get_ref(), b"0123xy6789");
    assert_eq!((source.reads, source.writes), (1, 1));
}
