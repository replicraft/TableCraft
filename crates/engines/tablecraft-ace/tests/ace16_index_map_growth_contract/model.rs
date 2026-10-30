//! Independent source bytes; shared literal fixtures are read-only inputs.
use super::{fixed_model, fixture, support};
use tablecraft_testkit::access::map_growth::model::{declaration_end, number};

pub const PAGE: usize = 4096;
pub const EOF: usize = 600;
pub const GLOBAL_BITMAP: usize = 599;
pub const TABLE: usize = 10;
pub const OTHER: usize = 580;
pub const OTHER_MAP: usize = 581;
pub const ROWS: usize = 900;
pub const ROOTS: [usize; 2] = [13, 19];
pub const MAPS: [usize; 2] = [14, 20];
pub const DATA: [usize; 5] = [12, 15, 16, 17, 18];
pub const PER_PAGE: usize = 200;
pub const MAP_BYTES: usize = 69;
pub const MAP_HEADER: usize = 5;
pub const BITMAP_HEADER: usize = 4;
pub const TABLE_USAGE: usize = 55;
pub const TABLE_HINT: usize = 59;
pub const TABLE_ROWS: usize = 16;
pub const DIRECTORY: usize = 14;
pub const SLOT_MASK: usize = 0x1fff;
const DEFINITION_HEADER: usize = 63;
const COLUMN_COUNT: usize = 45;
const LOGICAL_COUNT: usize = 47;
const PHYSICAL_COUNT: usize = 51;
const COUNTER_BYTES: usize = 12;
const COLUMN_BYTES: usize = 25;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const PHYSICAL_USAGE: usize = 34;
const CATALOG: usize = 2;
const CATALOG_ROWS: usize = 4;
const RELATIONSHIPS: usize = 7;
const OTHER_HISTORY_COLUMN: u16 = 7;

#[derive(Clone)]
pub struct Model {
    pub bytes: Vec<u8>,
    pub other_declarations: usize,
    pub selected_map_ranges: [(usize, usize); 2],
}

pub fn install(bytes: &mut [u8], page: usize, payload: &[u8]) {
    assert_eq!(payload.len(), PAGE);
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(payload);
}

pub fn row_range(bytes: &[u8], page: usize, slot: usize) -> (usize, usize) {
    let base = page * PAGE;
    assert!(slot < number(bytes, base + 12, 2));
    let word = number(bytes, base + DIRECTORY + slot * 2, 2);
    assert_eq!(word & !SLOT_MASK, 0);
    let start = base + word;
    let end = base
        + if slot == 0 {
            PAGE
        } else {
            number(bytes, base + DIRECTORY + (slot - 1) * 2, 2) & SLOT_MASK
        };
    assert!(start < end);
    (start, end)
}

pub fn map(members: &[usize]) -> Vec<u8> {
    let mut result = vec![0; MAP_BYTES];
    for &page in members {
        assert!(page < (MAP_BYTES - MAP_HEADER) * 8);
        result[MAP_HEADER + page / 8] |= 1 << (page % 8);
    }
    result
}

fn complete_empty(bytes: &mut [u8], definition: usize) {
    let end = declaration_end(bytes, definition);
    support::put16(bytes, end, u16::MAX);
}

fn add_other(bytes: &mut [u8]) -> usize {
    let mut rows = Vec::new();
    for slot in 0..3 {
        let (start, end) = row_range(bytes, CATALOG_ROWS, slot);
        rows.push((0, bytes[start..end].to_vec()));
    }
    rows.push((0, support::record(OTHER as i32, 0, 1, 0, b"\xff\xfeOther")));
    install(
        bytes,
        CATALOG_ROWS,
        &support::data_page(CATALOG as u32, &rows),
    );
    support::put32(bytes, CATALOG * PAGE + TABLE_ROWS, rows.len() as u32);
    let mut definition = fixture::definition(&[("Id", 4, 0, 4)], 0, (OTHER_MAP as u32) << 8);
    support::put32(&mut definition, TABLE_HINT, ((OTHER_MAP as u32) << 8) | 1);
    install(bytes, OTHER, &definition);
    install(
        bytes,
        OTHER_MAP,
        &support::data_page(
            0,
            &[(0, map(&[])), (0, map(&[])), (0, map(&[])), (0, map(&[]))],
        ),
    );
    complete_empty(bytes, OTHER);
    declaration_end(bytes, OTHER)
}

pub fn source(code: u8) -> Model {
    let base = fixed_model::source(code, fixed_model::Kind::Int32, vec![ROWS], false);
    let mut bytes = base.bytes;
    bytes.resize(EOF * PAGE, 0);
    for (physical, &container) in MAPS.iter().enumerate() {
        install(
            &mut bytes,
            container,
            &support::data_page(0, &[(0, map(&[ROOTS[physical]]))]),
        );
    }
    let mut global = vec![0; MAP_BYTES];
    global[0] = 1;
    support::put32(&mut global, 1, GLOBAL_BITMAP as u32);
    install(&mut bytes, 1, &support::data_page(1, &[(0, global)]));
    let mut bitmap = vec![0; PAGE];
    bitmap[..BITMAP_HEADER].copy_from_slice(&[5, 1, 0, 0]);
    for page in EOF..(PAGE - BITMAP_HEADER) * 8 {
        bitmap[BITMAP_HEADER + page / 8] |= 1 << (page % 8);
    }
    install(&mut bytes, GLOBAL_BITMAP, &bitmap);
    for definition in [CATALOG, RELATIONSHIPS, TABLE] {
        complete_empty(&mut bytes, definition);
    }
    let other_declarations = add_other(&mut bytes);
    let selected_map_ranges = MAPS.map(|page| row_range(&bytes, page, 0));
    for &(start, end) in &selected_map_ranges {
        assert_eq!(end - start, MAP_BYTES);
    }
    assert_eq!(number(&bytes, TABLE * PAGE + PHYSICAL_COUNT, 4), 2);
    assert_eq!(number(&bytes, TABLE * PAGE + LOGICAL_COUNT, 4), 4);
    Model {
        bytes,
        other_declarations,
        selected_map_ranges,
    }
}

pub fn bind_other_table_owned(model: &mut Model) {
    support::put32(
        &mut model.bytes,
        OTHER * PAGE + TABLE_USAGE,
        (MAPS[0] as u32) << 8,
    );
}

pub fn bind_other_table_hint(model: &mut Model) {
    support::put32(
        &mut model.bytes,
        OTHER * PAGE + TABLE_HINT,
        (MAPS[0] as u32) << 8,
    );
}

pub fn bind_historical_owned(model: &mut Model) {
    let start = model.other_declarations;
    support::put16(&mut model.bytes, start, OTHER_HISTORY_COLUMN);
    support::put32(&mut model.bytes, start + 2, (MAPS[0] as u32) << 8);
    support::put32(&mut model.bytes, start + 6, ((OTHER_MAP as u32) << 8) | 3);
    support::put16(&mut model.bytes, start + 10, u16::MAX);
}

pub fn share_container_distinct_rows(model: &mut Model) {
    install(
        &mut model.bytes,
        MAPS[0],
        &support::data_page(0, &[(0, map(&[ROOTS[0]])), (0, map(&[])), (0, map(&[]))]),
    );
    support::put32(
        &mut model.bytes,
        OTHER * PAGE + TABLE_USAGE,
        ((MAPS[0] as u32) << 8) | 1,
    );
    support::put32(
        &mut model.bytes,
        OTHER * PAGE + TABLE_HINT,
        ((MAPS[0] as u32) << 8) | 2,
    );
    assert_eq!(
        row_range(&model.bytes, MAPS[0], 0),
        model.selected_map_ranges[0]
    );
}

pub fn bind_other_bitmap_to_global(model: &mut Model) {
    let mut owned = vec![0; MAP_BYTES];
    owned[0] = 1;
    support::put32(&mut owned, 1, GLOBAL_BITMAP as u32);
    install(
        &mut model.bytes,
        OTHER_MAP,
        &support::data_page(
            0,
            &[(0, owned), (0, map(&[])), (0, map(&[])), (0, map(&[]))],
        ),
    );
}

pub fn selected_usage_position(bytes: &[u8], physical: usize) -> usize {
    let base = TABLE * PAGE;
    let columns = number(bytes, base + COLUMN_COUNT, 2);
    let count = number(bytes, base + PHYSICAL_COUNT, 4);
    let mut position = base + DEFINITION_HEADER + count * COUNTER_BYTES + columns * COLUMN_BYTES;
    for _ in 0..columns {
        position += 2 + number(bytes, position, 2);
    }
    position + physical * PHYSICAL_BYTES + PHYSICAL_USAGE
}

/// Keep the second raw physical slot but remove its logical names. This is a
/// metadata-census discriminator, not a claim that its foreign map is writable.
pub fn unreferenced_physical_alias(model: &mut Model) {
    let physical_start = selected_usage_position(&model.bytes, 0) - PHYSICAL_USAGE;
    let logical_start = physical_start + 2 * PHYSICAL_BYTES;
    let names_start = logical_start + 4 * LOGICAL_BYTES;
    let mut position = names_start;
    let mut names = Vec::new();
    for _ in 0..2 {
        let width = number(&model.bytes, position, 2);
        names.push(model.bytes[position..position + 2 + width].to_vec());
        position += 2 + width;
    }
    support::put32(&mut model.bytes, TABLE * PAGE + LOGICAL_COUNT, 2);
    let new_names = logical_start + 2 * LOGICAL_BYTES;
    model.bytes[new_names..(TABLE + 1) * PAGE].fill(0);
    position = new_names;
    for name in names {
        model.bytes[position..position + name.len()].copy_from_slice(&name);
        position += name.len();
    }
    support::put16(&mut model.bytes, position, u16::MAX);
    let usage = selected_usage_position(&model.bytes, 1);
    support::put32(&mut model.bytes, usage, (MAPS[0] as u32) << 8);
    assert_eq!(number(&model.bytes, TABLE * PAGE + PHYSICAL_COUNT, 4), 2);
    assert_eq!(number(&model.bytes, TABLE * PAGE + LOGICAL_COUNT, 4), 2);
}

pub fn unknown_other_history(model: &mut Model) {
    model.bytes[model.other_declarations..(OTHER + 1) * PAGE].fill(0);
}

pub fn assert_literal_source(model: &Model) {
    assert_eq!(model.bytes.len(), EOF * PAGE);
    assert_eq!(number(&model.bytes, TABLE * PAGE + TABLE_ROWS, 4), ROWS);
    for (physical, &root) in ROOTS.iter().enumerate() {
        let map_range = row_range(&model.bytes, MAPS[physical], 0);
        assert_eq!(map_range, model.selected_map_ranges[physical]);
        assert_eq!(&model.bytes[map_range.0..map_range.1], map(&[root]));
        let usage = selected_usage_position(&model.bytes, physical);
        assert_eq!(number(&model.bytes, usage, 4), MAPS[physical] << 8);
        assert_eq!(&model.bytes[root * PAGE..root * PAGE + 2], &[4, 1]);
        assert_eq!(number(&model.bytes, root * PAGE + 4, 4), TABLE);
        assert_eq!(number(&model.bytes, root * PAGE + 24, 2), 7);
    }
    for row in 0..ROWS {
        let page = DATA[row / PER_PAGE];
        let slot = row % PER_PAGE;
        let (start, end) = row_range(&model.bytes, page, slot);
        assert_eq!(end - start, 11);
        assert_eq!(number(&model.bytes, start, 2), 2);
        assert_eq!(number(&model.bytes, start + 2, 4), row + 1);
        assert_eq!(number(&model.bytes, start + 6, 4), 0);
        assert_eq!(model.bytes[end - 1], 3);
    }
    assert_eq!(
        &model.bytes[GLOBAL_BITMAP * PAGE..GLOBAL_BITMAP * PAGE + BITMAP_HEADER],
        &[5, 1, 0, 0],
    );
    for page in 0..EOF + 6 {
        let offset = GLOBAL_BITMAP * PAGE + BITMAP_HEADER + page / 8;
        let free = model.bytes[offset] & (1 << (page % 8)) != 0;
        assert_eq!(free, page >= EOF);
    }
    for definition in [CATALOG, RELATIONSHIPS, TABLE, OTHER] {
        assert_eq!(
            number(&model.bytes, declaration_end(&model.bytes, definition), 2),
            0xffff
        );
    }
}
