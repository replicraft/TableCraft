mod support;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use tablecraft_ace::{InspectError, read_table};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

struct FaultReader {
    cursor: Cursor<Vec<u8>>,
    operations: usize,
    fail_at: Option<usize>,
    chunk: usize,
    eof: bool,
}
impl FaultReader {
    fn new(bytes: &[u8], fail_at: Option<usize>) -> Self {
        Self {
            cursor: Cursor::new(bytes.to_vec()),
            operations: 0,
            fail_at,
            chunk: usize::MAX,
            eof: false,
        }
    }
    fn fails(&mut self) -> bool {
        self.operations += 1;
        self.fail_at == Some(self.operations)
    }
}
impl Read for FaultReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.fails() {
            return if self.eof {
                Ok(0)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "sensitive source details",
                ))
            };
        }
        let count = bytes.len().min(self.chunk);
        self.cursor.read(&mut bytes[..count])
    }
}
impl Seek for FaultReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.fails() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sensitive source details",
            ));
        }
        self.cursor.seek(position)
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn fixture() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
#[test]
fn every_callback_failure_aborts_a_chained_catalog_definition() {
    let mut bytes = support::image(false, false);
    support::put32(&mut bytes, 2 * 4096 + 8, 5000);
    support::put32(&mut bytes, 2 * 4096 + 4, 7);
    bytes.resize(8 * 4096, 0);
    bytes[7 * 4096..7 * 4096 + 2].copy_from_slice(&[2, 1]);
    let mut baseline = FaultReader::new(&bytes, None);
    assert_eq!(
        tablecraft_ace::inspect_catalog(
            &mut baseline,
            CatalogScope::new(1),
            &mut budget(),
            &|| false
        )
        .unwrap()
        .catalog()
        .objects()
        .len(),
        2
    );
    for fail_at in 1..=baseline.operations {
        let mut source = FaultReader::new(&bytes, Some(fail_at));
        assert!(matches!(
            tablecraft_ace::inspect_catalog(
                &mut source,
                CatalogScope::new(1),
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Io(io::ErrorKind::PermissionDenied))
        ));
        assert_eq!(source.operations, fail_at);
    }
}
#[test]
fn every_read_and_seek_failure_aborts_scalar_and_long_text_snapshots() {
    let bytes = fixture();
    for (name, rows) in [("ROULETTE_TB", 38), ("USysRibbons", 2)] {
        let mut baseline = FaultReader::new(&bytes, None);
        assert_eq!(
            read_table(
                &mut baseline,
                CatalogScope::new(1),
                name,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows()
            .len(),
            rows
        );
        for fail_at in 1..=baseline.operations {
            for eof in [false, true] {
                let mut source = FaultReader::new(&bytes, Some(fail_at));
                source.eof = eof;
                let error = read_table(
                    &mut source,
                    CatalogScope::new(1),
                    name,
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
                assert!(matches!(
                    error,
                    InspectError::Io(io::ErrorKind::PermissionDenied) | InspectError::Truncated
                ));
                assert!(!error.to_string().contains("sensitive"));
                assert_eq!(
                    source.operations, fail_at,
                    "must stop at the failing callback"
                );
                assert_eq!(source.cursor.get_ref(), &bytes);
            }
        }
    }
}
#[test]
fn short_reads_complete_and_failed_work_does_not_refund_budget() {
    let bytes = fixture();
    let mut short = FaultReader::new(&bytes, None);
    short.chunk = 17;
    assert_eq!(
        read_table(
            &mut short,
            CatalogScope::new(1),
            "USysRibbons",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows()
        .len(),
        2
    );
    let mut source = FaultReader::new(&bytes, Some(4));
    source.chunk = 17;
    let mut allowance = budget();
    assert_eq!(
        read_table(
            &mut source,
            CatalogScope::new(1),
            "USysRibbons",
            &mut allowance,
            &|| false
        )
        .err(),
        Some(InspectError::Io(io::ErrorKind::PermissionDenied))
    );
    assert!(allowance.remaining(Resource::Bytes) < budget().remaining(Resource::Bytes));
    assert!(allowance.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
}

#[test]
fn every_indexed_read_and_seek_failure_discards_the_snapshot() {
    let bytes = fixture();
    let mut baseline = FaultReader::new(&bytes, None);
    assert_eq!(
        tablecraft_ace::read_table_index(
            &mut baseline,
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows()
        .len(),
        38
    );
    for fail_at in 1..=baseline.operations {
        let mut reader = FaultReader::new(&bytes, Some(fail_at));
        assert_eq!(
            tablecraft_ace::read_table_index(
                &mut reader,
                CatalogScope::new(1),
                "ROULETTE_TB",
                "ORDR_NBR",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Io(io::ErrorKind::PermissionDenied))
        );
        assert_eq!(reader.operations, fail_at);
    }
}

#[test]
fn cancellation_after_index_traversal_prevents_row_publication() {
    use std::cell::Cell;
    struct Cancels<'a> {
        cursor: Cursor<Vec<u8>>,
        flag: &'a Cell<bool>,
    }
    impl Read for Cancels<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.cursor.read(bytes)
        }
    }
    impl Seek for Cancels<'_> {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            if position == SeekFrom::Start(728 * 4096) {
                self.flag.set(true);
            }
            self.cursor.seek(position)
        }
    }
    let flag = Cell::new(false);
    let mut source = Cancels {
        cursor: Cursor::new(fixture()),
        flag: &flag,
    };
    assert_eq!(
        tablecraft_ace::read_table_index(
            &mut source,
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut budget(),
            &|| flag.get()
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(flag.get());
}

#[test]
fn every_selected_range_read_and_seek_failure_aborts_without_partial_rows() {
    use std::ops::Bound::Included;
    use tablecraft_ace::{Int32IndexSelection as Selection, read_table_index_range};
    let bytes = fixture();
    for selection in [
        Selection::Null,
        Selection::Range {
            lower: Included(10),
            upper: Included(12),
        },
    ] {
        let mut baseline = FaultReader::new(&bytes, None);
        read_table_index_range(
            &mut baseline,
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            selection,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for fail_at in 1..=baseline.operations {
            let mut reader = FaultReader::new(&bytes, Some(fail_at));
            assert_eq!(
                read_table_index_range(
                    &mut reader,
                    CatalogScope::new(1),
                    "ROULETTE_TB",
                    "ORDR_NBR",
                    selection,
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Io(io::ErrorKind::PermissionDenied))
            );
            assert_eq!(reader.operations, fail_at);
        }
    }
}
