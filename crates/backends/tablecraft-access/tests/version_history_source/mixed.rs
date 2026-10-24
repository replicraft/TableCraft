//! Original mixed metadata: one supported and one independently qualified history.
use super::{PAGE_BYTES, complex_fixture, fixture, support};

pub(super) const OWNER_PAGE: usize = 10;
pub(super) const OTHER_FLAT_PAGE: usize = 27;
pub(super) const FIELD_START: usize = 63;
pub(super) const FIELD_BYTES: usize = 25;

fn put_page(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES].copy_from_slice(page);
}

fn other_mapping() -> Vec<u8> {
    let mut row = 5_u16.to_le_bytes().to_vec();
    for number in [8_i32, 10, 27, 24] {
        row.extend(number.to_le_bytes());
    }
    let start = row.len() as u16;
    row.extend("OtherHistory".encode_utf16().flat_map(u16::to_le_bytes));
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(start.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(0x1f);
    row
}

pub(super) fn database() -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize(30 * PAGE_BYTES, 0);
    let mut owner = fixture::definition(
        &[
            ("Id", 4, 0, 4),
            ("Score", 0x12, 4, 4),
            ("Notes", 12, 0, 0),
            ("OtherHistory", 0x12, 8, 4),
            ("Details", 12, 0, 0),
        ],
        3,
        11 << 8,
    );
    for (position, type_id) in [(1, 7), (3, 8)] {
        let at = FIELD_START + position * FIELD_BYTES;
        support::put32(&mut owner, at + 11, type_id);
        owner[at + 15] = 7;
        owner[at + 16] = 0x20;
    }
    put_page(&mut bytes, OWNER_PAGE, &owner);

    let parents = [10_i32, 20, 30]
        .into_iter()
        .enumerate()
        .map(|(ordinal, key)| {
            let mut row = 5_u16.to_le_bytes().to_vec();
            row.extend((ordinal as i32 + 1).to_le_bytes());
            row.extend(key.to_le_bytes());
            row.extend(key.to_le_bytes());
            for offset in [14_u16, 14, 14] {
                row.extend(offset.to_le_bytes());
            }
            row.extend(2_u16.to_le_bytes());
            row.push(0x0b);
            (0, row)
        })
        .collect::<Vec<_>>();
    put_page(&mut bytes, 12, &support::data_page(10, &parents));

    let objects = [
        ("MSysObjects", 2),
        ("MSysRelationships", 7),
        ("Items", 10),
        ("MSysComplexColumns", 14),
        ("ValueType", 17),
        ("Children", 20),
        ("OtherType", 24),
        ("OtherChildren", 27),
    ];
    let rows = objects
        .into_iter()
        .map(|(name, id)| {
            let encoded = [b"\xff\xfe".as_slice(), name.as_bytes()].concat();
            (0, support::record(id, 0, 1, 0, &encoded))
        })
        .collect::<Vec<_>>();
    support::put32(&mut bytes, 2 * PAGE_BYTES + 16, rows.len() as u32);
    put_page(&mut bytes, 4, &support::data_page(2, &rows));
    support::put32(&mut bytes, 14 * PAGE_BYTES + 16, 2);
    put_page(
        &mut bytes,
        16,
        &support::data_page(
            14,
            &[
                (0, complex_fixture::mapping(7, 10, 20, 17)),
                (0, other_mapping()),
            ],
        ),
    );
    put_page(
        &mut bytes,
        24,
        &fixture::definition(&[("Details", 12, 0, 0), ("Modified", 8, 0, 8)], 0, 25 << 8),
    );
    put_page(&mut bytes, 25, &support::data_page(1, &[(0, vec![0; 5])]));
    let mut flat = fixture::definition(
        &[
            ("Owner", 4, 0, 4),
            ("Details", 12, 0, 0),
            ("Modified", 8, 4, 8),
            ("ChildID", 4, 12, 4),
        ],
        0,
        28 << 8,
    );
    flat[FIELD_START + 16] = 8;
    flat[FIELD_START + 3 * FIELD_BYTES + 15] = 7;
    put_page(&mut bytes, OTHER_FLAT_PAGE, &flat);
    put_page(&mut bytes, 28, &support::data_page(1, &[(0, vec![0; 5])]));
    bytes
}
