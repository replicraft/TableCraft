use super::{links_fixture, support};

const PAGE_BYTES: usize = 4096;
const FORMAT_VERSION: usize = 20;
const CHILD_DEFINITION_PAGE: usize = 20;
const CHILD_DATA_PAGE: usize = 22;
const LIVE_ROW_COUNT: usize = 16;

pub fn database(code: u8, offset: i32) -> Vec<u8> {
    build(code, offset, false)
}

pub fn with_null_parent(code: u8) -> Vec<u8> {
    build(code, 0, true)
}

fn build(code: u8, offset: i32, parent_two_null: bool) -> Vec<u8> {
    let mut bytes = links_fixture::populated();
    bytes[FORMAT_VERSION] = code;
    let members = [
        (10_i32, Some(101_i32 + offset), 1_i32),
        (10, Some(100 + offset), 2),
        (10, Some(101 + offset), 3),
        (10, None, 4),
        (
            20,
            if parent_two_null {
                None
            } else {
                Some(200 + offset)
            },
            5,
        ),
    ];
    support::put32(
        &mut bytes,
        CHILD_DEFINITION_PAGE * PAGE_BYTES + LIVE_ROW_COUNT,
        members.len() as u32,
    );
    let rows = members
        .into_iter()
        .map(|(key, value, id)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(key.to_le_bytes());
            row.extend(value.unwrap_or_default().to_le_bytes());
            row.extend(id.to_le_bytes());
            row.push(if value.is_some() { 7 } else { 5 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_DATA_PAGE * PAGE_BYTES..(CHILD_DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(CHILD_DEFINITION_PAGE as u32, &rows));
    bytes
}
