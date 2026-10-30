use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

use std::io::Cursor;
use tablecraft_ace::{InspectError, TableSnapshot, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const OWNER_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const FLAGS_OFFSET: usize = 15;
const EXTENDED_FLAGS_OFFSET: usize = 16;

fn database(code: u8, streams: &[&[u8]]) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Short", 10, 0, 510), ("Memo", 11, 0, 0)],
        streams.len() as u32,
        11 << 8,
    );
    definition[FIELD_START + 2 * FIELD_BYTES] = 12;
    for ordinal in [1, 2] {
        definition[FIELD_START + ordinal * FIELD_BYTES + FLAGS_OFFSET] = 2;
        definition[FIELD_START + ordinal * FIELD_BYTES + EXTENDED_FLAGS_OFFSET] = 1;
    }
    bytes[OWNER_PAGE * PAGE_BYTES..(OWNER_PAGE + 1) * PAGE_BYTES].copy_from_slice(&definition);

    let rows = streams
        .iter()
        .enumerate()
        .map(|(index, stream)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend((index as i32 + 1).to_le_bytes());
            row.extend(*stream);
            let memo_start = row.len() as u16;
            row.extend((0x80000000 | stream.len() as u32).to_le_bytes());
            row.extend([0; 8]);
            row.extend(*stream);
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(memo_start.to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(2_u16.to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(OWNER_PAGE as u32, &rows));
    bytes
}

fn read(bytes: &[u8]) -> Result<TableSnapshot, InspectError> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(32 * 1024 * 1024, 16384, 1_000_000),
        &|| false,
    )
}

#[test]
fn ordinary_short_and_inline_memo_share_boundary_aware_compressed_decoding() {
    let streams = [
        &[0xff, 0xfe, 0, b'A', 0][..],
        &[0xff, 0xfe, b'A', 0, b'B', 0, b'C', 0, 0, b'D'][..],
        &[0xff, 0xfe, 0, 0x3e, 0xd8, 0x80, 0xdd, b'A', 0][..],
    ];
    for code in [2, 5, 6] {
        let values = read(&database(code, &streams)).unwrap();
        for (row, expected) in values.rows().iter().zip(["A", "ABCD", "🦀A"]) {
            assert_eq!(row[1], Value::Text(expected.into()));
            assert_eq!(row[2], Value::Text(expected.into()));
        }
    }
}

#[test]
fn late_odd_wide_tails_and_unpaired_surrogates_do_not_publish_a_snapshot() {
    for code in [2, 5, 6] {
        for malformed in [
            &[0xff, 0xfe, 0, b'A'][..],
            &[0xff, 0xfe, 0, 1, 0xd8][..],
            &[0xff, 0xfe, 0, 1, 0xdc][..],
        ] {
            let bytes = database(code, &[&[0xff, 0xfe, 0, b'A', 0], malformed]);
            assert!(read(&bytes).is_err());
        }
    }
}
