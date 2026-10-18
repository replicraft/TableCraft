use super::*;
use std::{
    cell::Cell,
    io::{self, Cursor},
};

struct Writer<'a> {
    bytes: Cursor<Vec<u8>>,
    mode: u8,
    wrote: &'a Cell<bool>,
}

impl Write for Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.wrote.set(true);
        match self.mode {
            0 => self.bytes.write(&bytes[..bytes.len().min(2)]),
            1 => Ok(0),
            2 => Ok(bytes.len() + 1),
            3 => Err(ErrorKind::Interrupted.into()),
            _ => Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "private payload",
            )),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("bounded write does not flush or sync")
    }
}

impl Seek for Writer<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn short_writes_preserve_offsets_and_leave_byte_charging_with_the_caller() {
    let wrote = Cell::new(false);
    let mut writer = Writer {
        bytes: Cursor::new(vec![0; 8]),
        mode: 0,
        wrote: &wrote,
    };
    let mut budget = ResourceBudget::new(0, 0, 10);

    write_exact_at(&mut writer, 2, b"four", &mut budget, &|| false).unwrap();
    assert_eq!(writer.bytes.into_inner(), b"\0\0four\0\0");
}

#[test]
fn zero_invalid_and_interrupted_writes_remain_typed_and_bounded() {
    for (mode, expected) in [
        (1, IoError::Io(ErrorKind::WriteZero)),
        (2, IoError::InvalidWriteCount),
        (4, IoError::Io(ErrorKind::PermissionDenied)),
    ] {
        let wrote = Cell::new(false);
        let mut writer = Writer {
            bytes: Cursor::new(Vec::new()),
            mode,
            wrote: &wrote,
        };
        let error = write_exact_at(
            &mut writer,
            0,
            b"bytes",
            &mut ResourceBudget::new(0, 0, 10),
            &|| false,
        )
        .unwrap_err();

        assert_eq!(error, expected);
        assert!(!error.to_string().contains("private payload"));
    }

    let wrote = Cell::new(false);
    let mut writer = Writer {
        bytes: Cursor::new(Vec::new()),
        mode: 3,
        wrote: &wrote,
    };
    assert!(matches!(
        write_exact_at(
            &mut writer,
            0,
            b"bytes",
            &mut ResourceBudget::new(0, 0, 5),
            &|| false
        ),
        Err(IoError::Limit(_))
    ));
}

#[test]
fn cancellation_after_a_successful_write_is_still_reported() {
    let wrote = Cell::new(false);
    let mut writer = Writer {
        bytes: Cursor::new(Vec::new()),
        mode: 0,
        wrote: &wrote,
    };

    assert_eq!(
        write_exact_at(
            &mut writer,
            0,
            b"ok",
            &mut ResourceBudget::new(0, 0, 10),
            &|| wrote.get()
        ),
        Err(IoError::Cancelled)
    );
    assert_eq!(writer.bytes.into_inner(), b"ok");
}
