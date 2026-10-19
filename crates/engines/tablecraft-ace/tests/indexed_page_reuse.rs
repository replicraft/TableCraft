#[path = "mutation/fixture.rs"]
mod base;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
    ops::Bound::Included,
};
use tablecraft_ace::{InspectError, Int32IndexSelection, read_table_index, read_table_index_range};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

struct Traced {
    cursor: Cursor<Vec<u8>>,
    reads: Vec<u64>,
    chunk: usize,
    fail_page_read: Option<(u32, usize)>,
}
impl Traced {
    fn new(bytes: &[u8], chunk: usize) -> Self {
        Self {
            cursor: Cursor::new(bytes.to_vec()),
            reads: Vec::new(),
            chunk,
            fail_page_read: None,
        }
    }
    fn page_reads(&self, page: u32) -> usize {
        self.reads
            .iter()
            .filter(|&&at| at == u64::from(page) * 4096)
            .count()
    }
}
impl Read for Traced {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let at = self.cursor.position();
        self.reads.push(at);
        if let Some((page, count)) = self.fail_page_read
            && at == u64::from(page) * 4096
            && self.page_reads(page) == count
        {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let count = bytes.len().min(self.chunk);
        self.cursor.read(&mut bytes[..count])
    }
}
impl Seek for Traced {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn fixture() -> Vec<u8> {
    let rows = vec![
        vec![Some(1), Some(10)],
        vec![Some(2), Some(20)],
        vec![Some(3), Some(30)],
    ];
    let mut bytes = numeric_fixture::database(6, &[4, 4], &[(1, false)], &rows, 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    bytes
}
fn row(id: i32, score: i32) -> Vec<u8> {
    let mut bytes = 2_u16.to_le_bytes().to_vec();
    bytes.extend(id.to_le_bytes());
    bytes.extend(score.to_le_bytes());
    bytes.push(3);
    bytes
}
fn read<R: Read + Seek>(source: &mut R) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
}
fn assert_rows(table: &tablecraft_ace::TableSnapshot) {
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)]
        ]
    );
}

#[test]
fn consecutive_references_reuse_one_root_page_and_each_operation_starts_fresh() {
    let bytes = fixture();
    for chunk in [usize::MAX, 17] {
        let mut source = Traced::new(&bytes, chunk);
        assert_rows(&read(&mut source).unwrap());
        assert_eq!(source.page_reads(12), 1);
        assert_rows(&read(&mut source).unwrap());
        assert_eq!(source.page_reads(12), 2);
        let mut empty = Traced::new(&bytes, chunk);
        let table = read_table_index_range(
            &mut empty,
            CatalogScope::new(1),
            "Items",
            "Compound",
            Int32IndexSelection::Range {
                lower: Included(99),
                upper: Included(99),
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(table.rows().is_empty());
        assert_eq!(empty.page_reads(12), 0);
    }
}

#[test]
fn returning_to_an_earlier_data_page_fetches_it_again_without_reordering_rows() {
    let mut bytes = fixture();
    bytes.resize(16 * 4096, 0);
    bytes[11 * 4096 + 4095] = 0x90;
    bytes[12 * 4096..13 * 4096]
        .copy_from_slice(&support::data_page(10, &[(0, row(1, 10)), (0, row(3, 30))]));
    bytes[15 * 4096..16 * 4096].copy_from_slice(&support::data_page(10, &[(0, row(2, 20))]));
    for (i, (page, slot)) in [(12_u32, 0), (15, 0), (12, 1)].into_iter().enumerate() {
        let at = 14 * 4096 + 480 + i * 9 + 5;
        bytes[at..at + 4].copy_from_slice(&((page << 8) | slot).to_be_bytes());
    }
    let mut source = Traced::new(&bytes, usize::MAX);
    assert_rows(&read(&mut source).unwrap());
    assert_eq!(source.page_reads(12), 2);
    assert_eq!(source.page_reads(15), 1);
}

#[test]
fn relocation_targets_still_require_each_physical_read_and_full_validation() {
    let mut bytes = fixture();
    bytes.resize(16 * 4096, 0);
    bytes[11 * 4096 + 4095] = 0x90;
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(
        10,
        &[
            (0x4000, (15_u32 << 8).to_le_bytes().to_vec()),
            (0x4000, ((15_u32 << 8) | 1).to_le_bytes().to_vec()),
            (0, row(3, 30)),
        ],
    ));
    bytes[15 * 4096..16 * 4096].copy_from_slice(&support::data_page(
        10,
        &[(0x8000, row(1, 10)), (0x8000, row(2, 20))],
    ));
    let mut source = Traced::new(&bytes, usize::MAX);
    assert_rows(&read(&mut source).unwrap());
    assert_eq!(source.page_reads(12), 1);
    assert_eq!(source.page_reads(15), 2);
    let mut failed = Traced::new(&bytes, usize::MAX);
    failed.fail_page_read = Some((15, 2));
    assert_eq!(
        read(&mut failed).err(),
        Some(InspectError::Io(io::ErrorKind::PermissionDenied))
    );
    for change in [0, 4, 12] {
        let mut invalid = bytes.clone();
        invalid[15 * 4096 + change] = 0;
        assert!(read(&mut Cursor::new(invalid)).is_err());
    }
    let mut same_page = fixture();
    same_page[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(
        10,
        &[
            (0x4000, ((12_u32 << 8) | 3).to_le_bytes().to_vec()),
            (0, row(2, 20)),
            (0, row(3, 30)),
            (0x8000, row(1, 10)),
        ],
    ));
    let mut source = Traced::new(&same_page, usize::MAX);
    assert_rows(&read(&mut source).unwrap());
    assert_eq!(
        source.page_reads(12),
        2,
        "same-page relocation still fetches its physical target"
    );
}

#[test]
fn later_cached_slots_and_every_cancellation_point_still_abort_the_snapshot() {
    let bytes = fixture();
    let calls = Cell::new(0);
    let table = read_table_index(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_rows(&table);
    for fail_at in 1..=calls.get() {
        let seen = Cell::new(0);
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut budget(),
                &|| {
                    seen.set(seen.get() + 1);
                    seen.get() == fail_at
                }
            )
            .err(),
            Some(InspectError::Cancelled)
        );
        assert_eq!(seen.get(), fail_at);
    }
    for flags in [0_u16, 0x8000] {
        let mut invalid = bytes.clone();
        let original =
            u16::from_le_bytes(invalid[12 * 4096 + 18..12 * 4096 + 20].try_into().unwrap());
        support::put16(
            &mut invalid,
            12 * 4096 + 18,
            if flags == 0 { 0 } else { original | flags },
        );
        assert_eq!(
            read(&mut Cursor::new(invalid)).err(),
            Some(InspectError::Malformed)
        );
    }
}
