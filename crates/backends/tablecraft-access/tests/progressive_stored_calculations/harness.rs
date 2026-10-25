use super::*;
use std::{
    io::{Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) const PAGE_BYTES: usize = 4096;
pub(super) const DATA_PAGE: usize = 12;
pub(super) const FORMAT_OFFSET: usize = 20;
pub(super) const ROW_DIRECTORY: usize = 14;
pub(super) const RESULT_OFFSET: usize = 6;
const TABLE_PAGE: usize = 10;
const FIRST_FIELD: usize = 63;
const FIELD_BYTES: usize = 25;
const FIELD_CAPACITY_OFFSET: usize = 23;
const CAPACITY_OFFSET: usize =
    TABLE_PAGE * PAGE_BYTES + FIRST_FIELD + FIELD_BYTES + FIELD_CAPACITY_OFFSET;

pub(super) const SCOPE: CatalogScope = CatalogScope::new(1);

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

pub(super) fn source<'a, R: NativeReadSource>(
    input: R,
    batch: u32,
    cancelled: &'a dyn Fn() -> bool,
    stored: bool,
) -> NativeSource<'a, R> {
    let source = NativeSource::new_with_snapshot_limits(
        input,
        SCOPE,
        budget(),
        cancelled,
        TableSnapshotLimits::new(1, batch).unwrap(),
    )
    .unwrap();
    if stored {
        source.with_stored_calculations().unwrap()
    } else {
        source
    }
}

pub(super) fn collect(
    source: &impl Source,
    indices: &[usize],
) -> Result<Vec<Vec<Value>>, QueryError> {
    let mut rows = Vec::new();
    source.scan_projected_progressive("Items", indices, &mut |row| {
        rows.push(
            (0..row.len())
                .map(|index| row.get(index).unwrap().clone())
                .collect(),
        );
        Ok(())
    })?;
    Ok(rows)
}

pub(super) fn buffered(
    source: &impl Source,
    indices: &[usize],
) -> Result<Vec<Vec<Value>>, QueryError> {
    let mut rows = Vec::new();
    source.scan_projected("Items", indices, None, &mut |row| {
        rows.push(
            (0..row.len())
                .map(|index| row.get(index).unwrap().clone())
                .collect(),
        );
        Ok(())
    })?;
    Ok(rows)
}

pub(super) struct Watching {
    data: Cursor<Vec<u8>>,
    pub forbidden: Rc<Cell<bool>>,
    pub changed_capacity: Rc<Cell<bool>>,
    pub data_reads: Rc<Cell<usize>>,
}

impl Watching {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            data: Cursor::new(bytes),
            forbidden: Rc::new(Cell::new(false)),
            changed_capacity: Rc::new(Cell::new(false)),
            data_reads: Rc::new(Cell::new(0)),
        }
    }
}

impl NativeReadSource for Watching {}

impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let position = self.data.position() as usize;
        if position / PAGE_BYTES == DATA_PAGE {
            assert!(
                !self.forbidden.get(),
                "metadata-only operation read user rows"
            );
            self.data_reads.set(self.data_reads.get() + 1);
        }
        let count = self.data.read(bytes)?;
        if self.changed_capacity.get() && (position..position + count).contains(&CAPACITY_OFFSET) {
            bytes[CAPACITY_OFFSET - position..CAPACITY_OFFSET - position + size_of::<u16>()]
                .copy_from_slice(&41_u16.to_le_bytes());
        }
        Ok(count)
    }
}

impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.data.seek(position)
    }
}
