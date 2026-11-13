//! Independent QueryDef creation bytes, never using a production mutation codec.
pub mod initialization;
pub mod local_tables;
use super::{Fragment, QUERY_ID, fragment_records};
use crate::fixtures::access::{
    canonical_creation, canonical_native17_creation, packed_record::append_record, put16, put32,
    query_expression_edits,
};
use tablecraft_core::values::LegacyDateTime;

const PAGE: usize = 4096;
const CATALOG_DEFINITION: usize = 2;
const CATALOG_DATA: usize = 4;
const QUERY_DEFINITION: usize = 7;
const QUERY_DATA: usize = 8;
const TABLES_ID: u32 = 0x0f00_0001;
const ORDER: [u8; 4] = [0, 0, 0, 1];

/// Complete original and independently authored new-object image.
pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub native_query_id: i32,
}

/// Append a new constant SELECT to the real nine-page saved-query source.
/// Existing Saved identity/fragments and all unrelated bytes remain unchanged.
pub fn source(destination: &str, alias: &str, value: i32, captured_at: LegacyDateTime) -> Source {
    let before = query_expression_edits::source();
    let native_query_id = QUERY_ID + 1;
    let expected = after(
        &before,
        native_query_id,
        destination,
        alias,
        value,
        captured_at,
    );
    Source {
        before,
        expected,
        native_query_id,
    }
}

/// Author one additional object with an explicit independent expected identity.
/// This does not call or duplicate the production identity allocator.
pub fn after(
    before: &[u8],
    native_query_id: i32,
    destination: &str,
    alias: &str,
    value: i32,
    captured_at: LegacyDateTime,
) -> Vec<u8> {
    assert_eq!(before.len(), 9 * PAGE);
    after_at(
        before,
        native_query_id,
        destination,
        alias,
        value,
        captured_at,
        QueryStorage {
            definition: QUERY_DEFINITION,
            data: QUERY_DATA,
        },
    )
}

#[derive(Clone, Copy)]
struct QueryStorage {
    definition: usize,
    data: usize,
}

fn after_at(
    before: &[u8],
    native_query_id: i32,
    destination: &str,
    alias: &str,
    value: i32,
    captured_at: LegacyDateTime,
    storage: QueryStorage,
) -> Vec<u8> {
    let QueryStorage {
        definition: query_definition,
        data: query_data,
    } = storage;
    // NO_CLONE: The raw AFTER is independently mutable while callers retain the
    // complete immutable BEFORE oracle for protected-byte/source comparisons.
    let mut expected = before.to_vec();
    let mut catalog = canonical_native17_creation::catalog_row(
        &canonical_creation::catalog_record(native_query_id as u32, TABLES_ID, destination, 5, 0),
    );
    if destination.starts_with('\u{feff}') {
        escape_short_text(&mut catalog, 0, 11, 3);
    }
    // Native17 fixed DATE fields are independently framed at these literal
    // offsets; both are present and carry the same explicitly captured bits.
    let timestamp = captured_at.to_bits().to_le_bytes();
    catalog[12..20].copy_from_slice(&timestamp);
    catalog[20..28].copy_from_slice(&timestamp);
    assert_eq!(&catalog[catalog.len() - 3..], &[0xbf, 0, 0]);
    append_record(
        &mut expected[CATALOG_DATA * PAGE..(CATALOG_DATA + 1) * PAGE],
        &catalog,
    );
    increase_rows(&mut expected, CATALOG_DEFINITION, 1);

    let expression = value.to_string();
    let fragments = [
        Fragment {
            object: Some(native_query_id),
            attribute: Some(0),
            order: Some(&ORDER),
            ..Default::default()
        },
        Fragment {
            object: Some(native_query_id),
            attribute: Some(1),
            flag: Some(1),
            order: Some(&ORDER),
            ..Default::default()
        },
        Fragment {
            object: Some(native_query_id),
            attribute: Some(3),
            flag: Some(0),
            order: Some(&ORDER),
            ..Default::default()
        },
        Fragment {
            object: Some(native_query_id),
            attribute: Some(6),
            expression: Some(&expression),
            name1: Some(alias),
            order: Some(&ORDER),
            ..Default::default()
        },
        Fragment {
            object: Some(native_query_id),
            attribute: Some(255),
            order: Some(&ORDER),
            ..Default::default()
        },
    ];
    for (ordinal, (_, mut row)) in fragment_records(&fragments).into_iter().enumerate() {
        if ordinal == 3 && alias.starts_with('\u{feff}') {
            escape_short_text(&mut row, 1, 4, 1);
        }
        append_record(
            &mut expected[query_data * PAGE..(query_data + 1) * PAGE],
            &row,
        );
    }
    increase_rows(&mut expected, query_definition, 5);
    expected
}

/// Rekey the existing Saved object and every one of its five fragments together.
/// The physical declarations/counts remain unchanged, including ID7 collisions
/// used to verify rejection at the actual catalog/source boundary.
pub fn with_query_identity(native_id: i32) -> Vec<u8> {
    let mut before = query_expression_edits::source();
    let object = row_start(&before, CATALOG_DATA, 8);
    assert_eq!(
        i32::from_le_bytes(before[object + 2..object + 6].try_into().unwrap()),
        QUERY_ID
    );
    put32(&mut before, object + 2, native_id as u32);
    for slot in 0..5 {
        let row = row_start(&before, QUERY_DATA, slot);
        assert_eq!(
            i32::from_le_bytes(before[row + 2..row + 6].try_into().unwrap()),
            QUERY_ID
        );
        put32(&mut before, row + 2, native_id as u32);
    }
    before
}

/// Give the existing empty Databases container a negative native identity.
/// Its role, parent, flags and body stay unchanged; no child references exist.
pub fn with_databases_identity(native_id: i32) -> Vec<u8> {
    let mut before = query_expression_edits::source();
    let object = row_start(&before, CATALOG_DATA, 2);
    assert_eq!(
        u32::from_le_bytes(before[object + 2..object + 6].try_into().unwrap()),
        0x0f00_0002
    );
    put32(&mut before, object + 2, native_id as u32);
    before
}

/// Add one physically valid fragment whose owner has no catalog Query object.
pub fn orphaned_fragment() -> Vec<u8> {
    let mut before = query_expression_edits::source();
    let fragment = Fragment {
        object: Some(QUERY_ID + 1),
        attribute: Some(6),
        expression: Some("1"),
        name1: Some("Orphan"),
        order: Some(&ORDER),
        ..Default::default()
    };
    let mut rows = fragment_records(&[fragment]);
    let (_, row) = rows.pop().unwrap();
    append_record(
        &mut before[QUERY_DATA * PAGE..(QUERY_DATA + 1) * PAGE],
        &row,
    );
    increase_rows(&mut before, QUERY_DEFINITION, 1);
    before
}

/// Keep the source readable while aliasing its two present-zero DATE spans.
/// A new writer must reject declaration overlap before serializing either field.
pub fn overlapping_catalog_dates() -> Vec<u8> {
    let mut before = query_expression_edits::source();
    let date_update_offset = CATALOG_DEFINITION * PAGE + 63 + 5 * 25 + 21;
    put16(&mut before, date_update_offset, 10);
    before
}

fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * PAGE + 14 + 2 * slot;
    let offset = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(offset & 0xe000, 0);
    page * PAGE + usize::from(offset)
}

fn increase_rows(bytes: &mut [u8], definition: usize, added: u32) {
    let at = definition * PAGE + 16;
    let count = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    put32(bytes, at, count.checked_add(added).unwrap());
}

/// A leading U+FEFF aliases the native compressed marker in Short Text. Insert
/// the literal marked-wide escape into this new raw record, preserving all
/// other payloads. Memo expression bytes use their separate inline framing.
fn escape_short_text(row: &mut Vec<u8>, variable: usize, variables: usize, mask_bytes: usize) {
    let count_start = row.len() - mask_bytes - 2;
    assert_eq!(
        u16::from_le_bytes(row[count_start..count_start + 2].try_into().unwrap()) as usize,
        variables
    );
    let start_at = count_start - 2 * (variable + 1);
    let start = usize::from(u16::from_le_bytes(
        row[start_at..start_at + 2].try_into().unwrap(),
    ));
    assert_eq!(&row[start..start + 2], &[0xff, 0xfe]);
    row.splice(start..start, [0xff, 0xfe, 0]);
    let count_start = count_start + 3;
    for ordinal in variable + 1..=variables {
        let at = count_start - 2 * (ordinal + 1);
        let offset = u16::from_le_bytes(row[at..at + 2].try_into().unwrap());
        put16(row, at, offset.checked_add(3).unwrap());
    }
}
