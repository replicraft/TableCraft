use super::{fixture, index_fixture, support};
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, NativeImage, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

pub const PAGE_BYTES: usize = 4096;
pub const ORIGINAL_PAGES: usize = 24;
pub const DEFINITION: usize = 10;
pub const DATA_USAGE: usize = 11;
pub const DATA_PAGES: [usize; 2] = [12, 15];
pub const ROOTS: [usize; 2] = [13, 19];
pub const INDEX_MAPS: [usize; 2] = [14, 20];
pub const SLOT_START: usize = 14;
pub const SLOT_BYTES: usize = 2;
pub const DATA_ROW_COUNT: usize = 12;
pub const DATA_FREE: usize = 2;
pub const DATA_MASK: u16 = 0x1fff;
pub const ROWS_PER_PAGE: usize = 255;
pub const ENTRY_START: usize = 480;
pub const ENTRY_MAP: usize = 27;
pub const PREFIX: usize = 24;
pub const HEIGHT: usize = 26;
pub const OWNER: usize = 4;
pub const PREVIOUS: usize = 12;
pub const NEXT: usize = 16;
pub const TAIL: usize = 20;
pub const INDEX_FREE: usize = 2;
pub const MAP_HEADER: usize = 5;
const TABLE_HEADER: usize = 63;
const INDEX_COUNTER_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const KEY_START: usize = 4;
const KEY_BYTES: usize = 3;
const KEY_SLOTS: usize = 10;
const USAGE_OFFSET: usize = 34;
const ROOT_OFFSET: usize = 38;
const FLAGS_OFFSET: usize = 46;
const LOGICAL_NUMBER: usize = 4;
const PHYSICAL_NUMBER: usize = 8;
const MAP_PAYLOAD_BYTES: usize = 69;
const MAP_COVERAGE: usize = 512;
const TABLE_ROW_COUNT: usize = 16;
const LOGICAL_COUNT: usize = 47;
const PHYSICAL_COUNT: usize = 51;
const ROW_SCORE_OFFSET: usize = 6;

pub struct Model {
    pub bytes: Vec<u8>,
    pub scores: Vec<Option<i32>>,
    pub directions: Vec<bool>,
    pub aliases: bool,
}

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

pub fn get16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + SLOT_BYTES].try_into().unwrap())
}

pub fn get32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub fn reference(row: usize) -> u32 {
    let page = DATA_PAGES[row / ROWS_PER_PAGE];
    ((page as u32) << 8) | (row % ROWS_PER_PAGE) as u32
}

pub fn wire(row: usize, score: Option<i32>, descending: bool) -> Vec<u8> {
    let mut key = Vec::new();
    match score {
        None => key.push(if descending { 255 } else { 0 }),
        Some(value) => {
            key.push(if descending { 128 } else { 127 });
            let biased = ((value as u32) ^ 0x80000000).to_be_bytes();
            key.extend(biased.map(|byte| if descending { !byte } else { byte }));
        }
    }
    key.extend(reference(row).to_be_bytes());
    key
}

pub fn ordered(scores: &[Option<i32>], descending: bool) -> Vec<Vec<u8>> {
    let mut entries = scores
        .iter()
        .copied()
        .enumerate()
        .map(|(row, score)| wire(row, score, descending))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

pub fn packet(entries: &[Vec<u8>], prefix: usize, height: u8) -> Vec<u8> {
    let mut page = vec![0xa7; PAGE_BYTES];
    page[..ENTRY_START].fill(0);
    page[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut page, OWNER, DEFINITION as u32);
    support::put16(&mut page, PREFIX, prefix as u16);
    page[HEIGHT] = height;
    let mut length = 0;
    for (position, entry) in entries.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(&entry[..shared], &entries[0][..shared]);
        let encoded = &entry[shared..];
        assert!(length + encoded.len() <= PAGE_BYTES - ENTRY_START);
        page[ENTRY_START + length..ENTRY_START + length + encoded.len()].copy_from_slice(encoded);
        length += encoded.len();
        page[ENTRY_MAP + length / 8] |= 1 << (length % 8);
    }
    support::put16(
        &mut page,
        INDEX_FREE,
        (PAGE_BYTES - ENTRY_START - length) as u16,
    );
    page
}

pub fn entries(bytes: &[u8], number: usize) -> Vec<Vec<u8>> {
    let page = &bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES];
    let prefix = get16(page, PREFIX) as usize;
    let mut start = 0;
    let mut result: Vec<Vec<u8>> = Vec::new();
    for end in 1..=PAGE_BYTES - ENTRY_START {
        if page[ENTRY_MAP + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let mut entry = if result.is_empty() {
            Vec::new()
        } else {
            result[0][..prefix].to_vec()
        };
        entry.extend_from_slice(&page[ENTRY_START + start..ENTRY_START + end]);
        result.push(entry);
        start = end;
    }
    result
}

pub fn map_byte(bytes: &[u8], page: usize, number: usize) -> usize {
    let start = get16(bytes, page * PAGE_BYTES + SLOT_START) as usize & DATA_MASK as usize;
    page * PAGE_BYTES + start + MAP_HEADER + number / 8
}

pub fn data_offset(bytes: &[u8], row: usize) -> usize {
    let page = DATA_PAGES[row / ROWS_PER_PAGE];
    let slot = row % ROWS_PER_PAGE;
    let start = get16(bytes, page * PAGE_BYTES + SLOT_START + slot * SLOT_BYTES) & DATA_MASK;
    page * PAGE_BYTES + usize::from(start) + ROW_SCORE_OFFSET
}

fn definition(scores: usize, directions: &[bool], aliases: bool, unique: bool) -> Vec<u8> {
    let fields = fixture::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4)],
        scores as u32,
        (DATA_USAGE as u32) << 8,
    );
    let physical = directions.len();
    let logical = physical * if aliases { 2 } else { 1 };
    let columns_start = TABLE_HEADER + physical * INDEX_COUNTER_BYTES;
    let fields_end = TABLE_HEADER + 2 * COLUMN_BYTES;
    let field_name_bytes = 18;
    let index_start = columns_start + 2 * COLUMN_BYTES + field_name_bytes;
    let logical_start = index_start + physical * PHYSICAL_BYTES;
    let mut page = vec![0; PAGE_BYTES];
    page[..TABLE_HEADER].copy_from_slice(&fields[..TABLE_HEADER]);
    page[columns_start..columns_start + 2 * COLUMN_BYTES]
        .copy_from_slice(&fields[TABLE_HEADER..fields_end]);
    page[columns_start + 2 * COLUMN_BYTES..index_start]
        .copy_from_slice(&fields[fields_end..fields_end + field_name_bytes]);
    support::put32(&mut page, LOGICAL_COUNT, logical as u32);
    support::put32(&mut page, PHYSICAL_COUNT, physical as u32);
    for (number, descending) in directions.iter().copied().enumerate() {
        support::put32(
            &mut page,
            TABLE_HEADER + number * INDEX_COUNTER_BYTES + 4,
            scores as u32,
        );
        let at = index_start + number * PHYSICAL_BYTES;
        for key in 0..KEY_SLOTS {
            support::put16(&mut page, at + KEY_START + key * KEY_BYTES, 0xffff);
        }
        support::put16(&mut page, at + KEY_START, 1);
        page[at + KEY_START + SLOT_BYTES] = u8::from(!descending);
        support::put32(
            &mut page,
            at + USAGE_OFFSET,
            (INDEX_MAPS[number] as u32) << 8,
        );
        support::put32(&mut page, at + ROOT_OFFSET, ROOTS[number] as u32);
        page[at + FLAGS_OFFSET] = if unique { 0x81 } else { 0x80 };
    }
    let mut names = logical_start + logical * LOGICAL_BYTES;
    for number in 0..logical {
        let physical = number / if aliases { 2 } else { 1 };
        let at = logical_start + number * LOGICAL_BYTES;
        support::put32(&mut page, at + LOGICAL_NUMBER, number as u32);
        support::put32(&mut page, at + PHYSICAL_NUMBER, physical as u32);
        let name = index_name(physical, aliases && number % 2 == 1);
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut page, names, encoded.len() as u16);
        names += SLOT_BYTES;
        page[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    page
}

pub fn index_name(physical: usize, alias: bool) -> &'static str {
    match (physical, alias) {
        (0, false) => "ScoreKey",
        (0, true) => "ScoreAlias",
        (1, false) => "ReverseKey",
        (1, true) => "ReverseAlias",
        _ => unreachable!(),
    }
}

pub fn database(
    code: u8,
    scores: Vec<Option<i32>>,
    directions: Vec<bool>,
    aliases: bool,
    unique: bool,
    rule: bool,
) -> Model {
    assert!(!scores.is_empty() && scores.len() <= 2 * ROWS_PER_PAGE);
    let mut bytes = index_fixture::with_rule(
        false,
        true,
        true,
        if rule { "ValidationRule" } else { "Caption" },
        if rule {
            "[Score] < 2147483647"
        } else {
            "split contract"
        },
    );
    bytes.resize(ORIGINAL_PAGES * PAGE_BYTES, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let table = definition(scores.len(), &directions, aliases, unique);
    bytes[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES].copy_from_slice(&table);
    let mut owned = vec![0; MAP_PAYLOAD_BYTES];
    for (group, chunk) in scores.chunks(ROWS_PER_PAGE).enumerate() {
        let page = DATA_PAGES[group];
        owned[MAP_HEADER + page / 8] |= 1 << (page % 8);
        let mut rows = Vec::new();
        for (slot, score) in chunk.iter().copied().enumerate() {
            let ordinal = group * ROWS_PER_PAGE + slot;
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((ordinal as i32 + 1).to_le_bytes());
            row.extend(score.unwrap_or(0).to_le_bytes());
            row.push(if score.is_some() { 3 } else { 1 });
            rows.push((0, row));
        }
        let mut data = support::data_page(DEFINITION as u32, &rows);
        let bytes_used: usize = rows.iter().map(|(_, row)| row.len() + SLOT_BYTES).sum();
        support::put16(
            &mut data,
            DATA_FREE,
            (PAGE_BYTES - SLOT_START - bytes_used) as u16,
        );
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES].copy_from_slice(&data);
    }
    bytes[DATA_USAGE * PAGE_BYTES..(DATA_USAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let mut global = vec![0; MAP_PAYLOAD_BYTES];
    for number in ORIGINAL_PAGES..MAP_COVERAGE {
        global[MAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, global)]));
    for (number, descending) in directions.iter().copied().enumerate() {
        let values = ordered(&scores, descending);
        let mut prefix = values[0].len();
        for entry in &values {
            prefix = prefix.min(
                entry
                    .iter()
                    .zip(&values[0])
                    .take_while(|(left, right)| left == right)
                    .count(),
            );
        }
        let root = ROOTS[number];
        bytes[root * PAGE_BYTES..(root + 1) * PAGE_BYTES]
            .copy_from_slice(&packet(&values, prefix, 0));
        let mut map = vec![0; MAP_PAYLOAD_BYTES];
        map[MAP_HEADER + root / 8] |= 1 << (root % 8);
        let map_page = INDEX_MAPS[number];
        bytes[map_page * PAGE_BYTES..(map_page + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(0, &[(0, map)]));
    }
    Model {
        bytes,
        scores,
        directions,
        aliases,
    }
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

pub fn checked(model: &Model) {
    let report = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(
        get32(&model.bytes, DEFINITION * PAGE_BYTES + TABLE_ROW_COUNT) as usize,
        model.scores.len()
    );
    for (group, chunk) in model.scores.chunks(ROWS_PER_PAGE).enumerate() {
        assert_eq!(
            get16(
                &model.bytes,
                DATA_PAGES[group] * PAGE_BYTES + DATA_ROW_COUNT
            ) as usize,
            chunk.len()
        );
    }
}
