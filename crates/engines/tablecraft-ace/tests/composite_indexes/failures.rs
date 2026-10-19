use super::*;
use std::{
    cell::Cell,
    io::{self, Read, Seek, SeekFrom},
};

struct FaultSource<'a> {
    cursor: Cursor<&'a [u8]>,
    calls: usize,
    fail: Option<usize>,
}
impl FaultSource<'_> {
    fn check(&mut self) -> io::Result<()> {
        self.calls += 1;
        if self.fail == Some(self.calls) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Ok(())
    }
}
impl Read for FaultSource<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        self.cursor.read(bytes)
    }
}
impl Seek for FaultSource<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.check()?;
        self.cursor.seek(position)
    }
}

#[test]
fn every_io_and_cancellation_failure_aborts_complete_compound_reads() {
    let bytes = fixture::database(
        6,
        &[5, 0x13],
        &[(0, false), (1, true)],
        &[
            vec![Some(-1), None],
            vec![None, Some(i64::MAX)],
            vec![Some(1), Some(i64::MIN)],
        ],
        0,
    );
    let mut source = FaultSource {
        cursor: Cursor::new(bytes.as_slice()),
        calls: 0,
        fail: None,
    };
    let calls = Cell::new(0);
    let cancellation = || {
        calls.set(calls.get() + 1);
        false
    };
    assert_eq!(
        read_table_index(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &cancellation
        )
        .unwrap()
        .rows()
        .len(),
        3
    );
    for stop in 1..=source.calls {
        let mut failed = FaultSource {
            cursor: Cursor::new(bytes.as_slice()),
            calls: 0,
            fail: Some(stop),
        };
        assert_eq!(
            read_table_index(
                &mut failed,
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Io(io::ErrorKind::PermissionDenied))
        );
        assert_eq!(failed.calls, stop);
    }
    let total = calls.get();
    for stop in 1..=total {
        let calls = Cell::new(0);
        let cancellation = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut budget(),
                &cancellation
            )
            .err(),
            Some(InspectError::Cancelled)
        );
        assert_eq!(calls.get(), stop);
    }
}
