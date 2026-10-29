//! Adapt actual independent source/expected owners with explicit AutoNumber facts.
use crate::table::mutation::insert::tests::{
    long_value_inserts::{Family, declarations::model as missing},
    ordinary_rows::{fixture, model::Row},
};
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;

pub(super) struct Ordinary {
    pub case: fixture::Case,
    pub added: Vec<Row>,
    pub expected: Vec<u8>,
}

pub(super) fn ordinary(code: u8) -> Ordinary {
    let mut case = fixture::source(code, true, false, fixture::Index::Alias, false);
    let mut rich = Row::rich(10);
    rich.later_text = Some("Z".repeat(255));
    rich.first_bytes = Some(vec![0x92; 255]);
    rich.score = Some("Ā".repeat(127));
    rich.last_bytes = Some(vec![0x73; 510]);
    let mut empty = Row::nulls(11);
    empty.later_text = Some(String::new());
    empty.score = Some(String::new());
    empty.first_bytes = Some(Vec::new());
    let added = vec![rich, empty];
    generated(&mut case.before, 2, 9);
    let mut expected = fixture::expected(&case, &added);
    generated(&mut expected, 2, 11);
    Ordinary {
        case,
        added,
        expected,
    }
}

pub(super) fn external(code: u8, family: Family, chained: bool) -> missing::Case {
    let value = match (family, chained) {
        (Family::Ole, false) => Value::Binary(vec![0x92; 65]),
        (Family::Ole, true) => Value::Binary(vec![0x92; 8145]),
        (Family::Memo, false) => Value::Text("Ā".repeat(33)),
        (Family::Memo, true) => Value::Text("Ā".repeat(4073)),
        (Family::CompressedMemo, false) => Value::Text("A".repeat(65)),
        (Family::CompressedMemo, true) => Value::Text("Ā".repeat(4073)),
    };
    // A valid sibling is retained; both short and chained values extend the TD
    // and allocate a new container before the independently modeled LVAL bodies.
    let mut case = missing::external_value(code, family, value, true, false);
    generated(&mut case.before, 0, 9);
    generated(&mut case.expected, 0, 10);
    case
}

fn generated(bytes: &mut [u8], column: usize, last: i32) {
    let physical =
        u32::from_le_bytes(bytes[DEFINITION + 51..DEFINITION + 55].try_into().unwrap()) as usize;
    let flags = DEFINITION + 63 + physical * 12 + column * 25 + 15;
    assert!(matches!(bytes[flags], 3 | 7));
    bytes[flags] = 7;
    bytes[DEFINITION + 20..DEFINITION + 24].copy_from_slice(&last.to_le_bytes());
}

pub(super) fn new_values_property(bytes: &mut [u8]) {
    // A genuine parsed property on the generated ID field, with opaque text.
    // This does not invent a numeric encoding for the native New Values mode.
    let mut properties = b"MR2\0".to_vec();
    properties.extend(26_u32.to_le_bytes());
    properties.extend(0x80_u16.to_le_bytes());
    properties.extend(18_u16.to_le_bytes());
    properties.extend("NewValues".encode_utf16().flat_map(u16::to_le_bytes));
    properties.extend(36_u32.to_le_bytes());
    properties.extend(1_u16.to_le_bytes());
    properties.extend(10_u32.to_le_bytes());
    properties.extend(4_u16.to_le_bytes());
    properties.extend("ID".encode_utf16().flat_map(u16::to_le_bytes));
    properties.extend([20, 0, 0, 10, 0, 0, 12, 0]);
    properties.extend("Random".encode_utf16().flat_map(u16::to_le_bytes));

    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    raw::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x80000000_u32 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend([16, 0, 2, 0, 0x3f]);
    let catalog = raw::data_page(
        2,
        &[
            (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
            (
                0,
                raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&catalog);
}
