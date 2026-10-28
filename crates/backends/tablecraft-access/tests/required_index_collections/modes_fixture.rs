//! Additional genuine records; existing fixture constructors remain unchanged.
use super::{fixture, history_fixture, modern_fixture, support};

const PAGE: usize = 4096;

pub fn decoded(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = fixture::source(code, descending, true);
    // Uncompressed native file wrapper: body20, version1, four UTF-16 units
    // including the terminator, type bin, and no file bytes.
    let empty_file = [
        0, 0, 0, 0, 20, 0, 0, 0, 20, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, b'b', 0, b'i', 0, b'n', 0, 0,
        0,
    ];
    let mut empty = modern_fixture::row(20, 3, 2, &[]);
    let old_footer = empty.len() - 17;
    let offsets = [14_u16, 18, 30, 30, 30, 42, 46];
    assert_eq!(old_footer, 46);
    empty.splice(18..30, modern_fixture::inline(&empty_file));
    let footer = empty.len() - 17;
    for (position, offset) in offsets.into_iter().rev().enumerate() {
        let shifted = if offset >= 30 { offset + 28 } else { offset };
        empty[footer + position * 2..footer + position * 2 + 2]
            .copy_from_slice(&shifted.to_le_bytes());
    }
    modern_fixture::install_rows(
        &mut bytes,
        &[
            modern_fixture::row(
                10,
                1,
                0,
                &modern_fixture::inline(super::old_attachment::PAYLOAD),
            ),
            modern_fixture::row(10, 2, 1, &[]),
            empty,
        ],
    );
    bytes
}

pub fn history(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = history_fixture::database_with(&[
        (10, 1, Some("older"), Some(0.25_f64.to_bits())),
        (10, 2, None, None),
        (20, 3, Some(""), Some(1.0_f64.to_bits())),
    ]);
    bytes[20] = code;
    bytes.resize(27 * PAGE, 0);
    let indexed = fixture::source(code, descending, true);
    // Three declared fields, one real physical/logical Id index. Retain the
    // history declaration's raw fields and names while inserting its counter
    // block and genuine ordinary index descriptors at their native positions.
    let original = bytes[10 * PAGE..11 * PAGE].to_vec();
    let definition = &mut bytes[10 * PAGE..11 * PAGE];
    definition.fill(0);
    definition[..63].copy_from_slice(&original[..63]);
    definition[63..75].copy_from_slice(&indexed[10 * PAGE + 63..10 * PAGE + 75]);
    definition[75..150].copy_from_slice(&original[63..138]);
    definition[150..180].copy_from_slice(&original[138..168]);
    definition[180..260].copy_from_slice(&indexed[10 * PAGE + 143..10 * PAGE + 223]);
    let name = "IdOrder"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    support::put16(definition, 260, name.len() as u16);
    definition[262..262 + name.len()].copy_from_slice(&name);
    support::put32(definition, 16, 4);
    support::put32(definition, 47, 1);
    support::put32(definition, 51, 1);
    for page in [11, 24, 25] {
        bytes[page * PAGE..(page + 1) * PAGE]
            .copy_from_slice(&indexed[page * PAGE..(page + 1) * PAGE]);
    }
    let rows = [
        (30_i32, Some(10_i32)),
        (10, None),
        (10, Some(20)),
        (20, Some(30)),
    ]
    .into_iter()
    .map(|(id, key)| {
        let mut row = 3_u16.to_le_bytes().to_vec();
        row.extend(id.to_le_bytes());
        row.extend(key.unwrap_or(777).to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(1_u16.to_le_bytes());
        row.push(if key.is_some() { 3 } else { 1 });
        (0, row)
    })
    .collect::<Vec<_>>();
    let mut logical = rows.clone();
    let mut stub = vec![0; 17];
    stub[..4].copy_from_slice(&(26_u32 << 8).to_le_bytes());
    logical[2] = (0x4000, stub);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &logical));
    bytes[26 * PAGE..27 * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, rows[2].1.clone())]));
    bytes
}
