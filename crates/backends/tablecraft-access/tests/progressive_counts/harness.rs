use super::{fixture, map_fixture, support};
use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::{NativeReadSource, TableSnapshotLimits};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub(super) const PAGE_BYTES: usize = 4096;
pub(super) const FIRST_DATA_PAGE: usize = map_fixture::FIRST_DATA_PAGE;
pub(super) const ORDINARY_DATA_PAGE: usize = 12;
pub(super) const TABLE_PAGE: usize = 10;
pub(super) const ROW_COUNT_OFFSET: usize = 16;
pub(super) const ROW_DIRECTORY_OFFSET: usize = 14;
pub(super) const DECIMAL_VALUE_OFFSET: usize = 6;
pub(super) const SCOPE: CatalogScope = CatalogScope::new(1);

const MAGIC_START: usize = 4;
const FORMAT_CODE_OFFSET: usize = 20;
const CATALOG_DEFINITION: usize = 2;
const CATALOG_DATA: usize = 4;
const MAP_PREFIX_BYTES: usize = 5;

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

pub(super) fn database(profile: u8, rows: usize) -> Vec<u8> {
    let mut bytes = map_fixture::database(rows, true, false);
    bytes[FORMAT_CODE_OFFSET] = profile;
    if profile != 1 {
        bytes[MAGIC_START..FORMAT_CODE_OFFSET].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes
}

pub(super) fn with_other(mut bytes: Vec<u8>) -> (Vec<u8>, usize) {
    let definition_page = bytes.len() / PAGE_BYTES;
    let usage_page = definition_page + 1;
    let data_page = usage_page + 1;
    bytes.resize((data_page + 1) * PAGE_BYTES, 0);
    support::put32(
        &mut bytes,
        CATALOG_DEFINITION * PAGE_BYTES + ROW_COUNT_OFFSET,
        4,
    );
    let catalog = support::data_page(
        CATALOG_DEFINITION as u32,
        &[
            (
                0,
                support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
            ),
            (
                0,
                support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, support::record(10, 0, 1, 0, b"\xff\xfeItems")),
            (
                0,
                support::record(definition_page as i32, 0, 1, 0, b"\xff\xfeOther"),
            ),
        ],
    );
    bytes[CATALOG_DATA * PAGE_BYTES..(CATALOG_DATA + 1) * PAGE_BYTES].copy_from_slice(&catalog);

    let definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4)],
        2,
        (usage_page as u32) << 8,
    );
    bytes[definition_page * PAGE_BYTES..usage_page * PAGE_BYTES].copy_from_slice(&definition);

    let mut bitmap = vec![0; (data_page + 1).div_ceil(8)];
    bitmap[data_page / 8] |= 1 << (data_page % 8);
    let map = [vec![0; MAP_PREFIX_BYTES], bitmap].concat();
    let usage = support::data_page(1, &[(0, map)]);
    bytes[usage_page * PAGE_BYTES..data_page * PAGE_BYTES].copy_from_slice(&usage);

    let rows = [(41_i32, 410_i32), (42, 420)]
        .into_iter()
        .map(|(identity, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(identity.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    let data = support::data_page(definition_page as u32, &rows);
    bytes[data_page * PAGE_BYTES..(data_page + 1) * PAGE_BYTES].copy_from_slice(&data);
    (bytes, data_page)
}

pub(super) struct Watched {
    input: Cursor<Vec<u8>>,
    pub pages: Rc<RefCell<Vec<usize>>>,
    pub failed_page: Rc<Cell<Option<usize>>>,
    pub cancel_page: Rc<Cell<Option<usize>>>,
    pub stopped: Rc<Cell<bool>>,
}

impl Watched {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            input: Cursor::new(bytes),
            pages: Rc::new(RefCell::new(Vec::new())),
            failed_page: Rc::new(Cell::new(None)),
            cancel_page: Rc::new(Cell::new(None)),
            stopped: Rc::new(Cell::new(false)),
        }
    }
}

impl NativeReadSource for Watched {}

impl Read for Watched {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let page = self.input.position() as usize / PAGE_BYTES;
        self.pages.borrow_mut().push(page);
        if self.failed_page.get() == Some(page) {
            return Err(std::io::Error::other("injected native read failure"));
        }

        let count = self.input.read(output)?;
        if self.cancel_page.get() == Some(page) {
            self.stopped.set(true);
        }
        Ok(count)
    }
}

impl Seek for Watched {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}

pub(super) fn source<'a>(
    input: Watched,
    cancelled: &'a dyn Fn() -> bool,
) -> NativeSource<'a, Watched> {
    NativeSource::new_with_snapshot_limits(
        input,
        SCOPE,
        budget(),
        cancelled,
        TableSnapshotLimits::new(1, 1).unwrap(),
    )
    .unwrap()
}
