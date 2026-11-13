//! Independent QueryDef creation bytes, never using a production mutation codec.
use super::{Fragment, QUERY_ID, fragment_records};
use crate::fixtures::access::{
    canonical_creation, canonical_native17_creation, packed_record::append_record, put32,
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
    let mut expected = before.to_vec();
    let mut catalog = canonical_native17_creation::catalog_row(
        &canonical_creation::catalog_record(native_query_id as u32, TABLES_ID, destination, 5, 0),
    );
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
    for (_, row) in fragment_records(&fragments) {
        append_record(
            &mut expected[QUERY_DATA * PAGE..(QUERY_DATA + 1) * PAGE],
            &row,
        );
    }
    increase_rows(&mut expected, QUERY_DEFINITION, 5);
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
