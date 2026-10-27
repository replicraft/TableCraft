use super::{fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, NativeImage, SlotScalarUpdate, check_table, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub const PAGE_BYTES: usize = 4096;
pub const DEFINITION_PAGE: usize = 10;
pub const MAP_PAGE: usize = 11;
pub const DATA_PAGE: usize = 12;
pub const FREE_OFFSET: usize = 2;
pub const OWNER_OFFSET: usize = 4;
pub const COUNT_OFFSET: usize = 12;
pub const SLOT_OFFSET: usize = 14;
pub const SLOT_BYTES: usize = 2;
pub const HIDDEN: u16 = 0x8000;
pub const FORWARDED: u16 = 0x4000;
pub const PHYSICAL_OFFSET: u16 = 0x1fff;
const FLAGS_OFFSET: usize = 63 + 25 + 15;
const VARIABLE_COUNT_OFFSET: usize = 43;
const TABLE_FREE_REFERENCE: usize = 59;
const GLOBAL_BITMAP_BYTES: usize = 69;
const BITMAP_DATA_OFFSET: usize = 5;
const GLOBAL_COVERAGE: usize = 512;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

pub fn record(value: &[u8]) -> Vec<u8> {
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(1_i32.to_le_bytes());
    row.extend(value);
    row.extend((6_u16 + value.len() as u16).to_le_bytes());
    row.extend(6_u16.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(3);
    row
}

pub fn page(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = support::data_page(DEFINITION_PAGE as u32, rows);
    let bodies: usize = rows.iter().map(|(_, row)| row.len()).sum();
    let free = PAGE_BYTES - bodies - SLOT_OFFSET - rows.len() * SLOT_BYTES;
    support::put16(&mut page, FREE_OFFSET, free as u16);
    page
}

pub fn image(rows: &[(u16, Vec<u8>)], live: u32, hint: bool) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Score", 9, 0, 255)],
        live,
        (MAP_PAGE as u32) << 8,
    );
    definition[FLAGS_OFFSET] = 2;
    support::put16(&mut definition, VARIABLE_COUNT_OFFSET, 1);
    support::put32(
        &mut definition,
        TABLE_FREE_REFERENCE,
        ((MAP_PAGE as u32) << 8) | 1,
    );
    bytes[DEFINITION_PAGE * PAGE_BYTES..(DEFINITION_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&definition);
    let owned = vec![0, 0, 0, 0, 0, 0, 0x10];
    let available = vec![0, 0, 0, 0, 0, 0, if hint { 0x10 } else { 0 }];
    bytes[MAP_PAGE * PAGE_BYTES..(MAP_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, owned), (0, available)]));
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(&page(rows));
    let mut global = vec![0; GLOBAL_BITMAP_BYTES];
    for number in bytes.len() / PAGE_BYTES..GLOBAL_COVERAGE {
        global[BITMAP_DATA_OFFSET + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    bytes
}

pub fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn preview(
    bytes: &[u8],
    expected: &[u8],
    value: &[u8],
) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_page_updates(
        "Items",
        "Score",
        &[SlotScalarUpdate::new(
            0,
            Value::Binary(expected.to_vec()),
            Value::Binary(value.to_vec()),
        )],
        &mut budget(),
        &|| false,
    )
}

pub fn read(bytes: &[u8]) -> Vec<Vec<Value>> {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

pub fn word(bytes: &[u8], slot: usize) -> u16 {
    let offset = DATA_PAGE * PAGE_BYTES + SLOT_OFFSET + SLOT_BYTES * slot;
    u16::from_le_bytes(bytes[offset..offset + SLOT_BYTES].try_into().unwrap())
}

pub fn count(bytes: &[u8]) -> u16 {
    let offset = DATA_PAGE * PAGE_BYTES + COUNT_OFFSET;
    u16::from_le_bytes(bytes[offset..offset + SLOT_BYTES].try_into().unwrap())
}
