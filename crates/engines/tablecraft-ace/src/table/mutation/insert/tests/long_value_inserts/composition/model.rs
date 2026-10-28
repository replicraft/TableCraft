//! Literal two-column storage, ordinary rows and index bytes precede the API.
use super::super::{Family, PAGE, external, frames, original, raw};
use crate::{LongValueCheckStatus, check_table_long_values, read_table, test_support::base};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[derive(Clone)]
pub(in crate::table::mutation::insert::tests::long_value_inserts) struct Row {
    pub id: i32,
    pub blob: Option<Vec<u8>>,
    pub memo: Option<String>,
}

impl Row {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            Value::Null,
            self.blob
                .as_ref()
                .map_or(Value::Null, |value| Value::Binary(value.clone())),
            self.memo
                .as_ref()
                .map_or(Value::Null, |value| Value::Text(value.clone())),
        ]
    }
}

pub(in crate::table::mutation::insert::tests::long_value_inserts) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub added: Vec<Row>,
    pub expected: Vec<u8>,
}

fn descriptor(bytes: &[u8], reference: Option<u32>) -> Vec<u8> {
    let tag = if reference.is_some() {
        0x40000000
    } else {
        0x80000000
    };
    let mut result = (tag | bytes.len() as u32).to_le_bytes().to_vec();
    result.extend(reference.unwrap_or(0).to_le_bytes());
    result.extend([0; 4]);
    if reference.is_none() {
        result.extend(bytes);
    }
    result
}

fn record(
    id: i32,
    fixed_offset: usize,
    blob: Option<&[u8]>,
    memo: Option<&[u8]>,
    old: bool,
) -> Vec<u8> {
    let fixed_end = (fixed_offset + 6).max(10);
    let mut result = vec![if old { 0xa7 } else { 0 }; fixed_end];
    result[..2].copy_from_slice(&4_u16.to_le_bytes());
    result[fixed_offset + 2..fixed_offset + 6].copy_from_slice(&id.to_le_bytes());
    result[6..10].fill(if old { 0x9d } else { 0 });
    let start = result.len() as u16;
    if let Some(memo) = memo {
        result.extend(memo);
    }
    let middle = result.len() as u16;
    if let Some(blob) = blob {
        result.extend(blob);
    }
    let end = result.len() as u16;
    result.extend(end.to_le_bytes());
    result.extend(middle.to_le_bytes());
    result.extend(start.to_le_bytes());
    result.extend(2_u16.to_le_bytes());
    result.push(1 | (u8::from(blob.is_some()) << 2) | (u8::from(memo.is_some()) << 3));
    result
}

fn data_page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(owner, rows);
    let used = rows.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    raw::put16(&mut page, 2, (PAGE - 14 - 2 * rows.len() - used) as u16);
    page
}

fn long_page(rows: &[Vec<u8>]) -> Vec<u8> {
    let rows = rows
        .iter()
        .cloned()
        .map(|body| (0, body))
        .collect::<Vec<_>>();
    let mut page = data_page(0, &rows);
    page[4..8].copy_from_slice(b"LVAL");
    page
}

fn definition(original: &[u8], count: usize, fixed_offset: usize, histories: [u32; 2]) -> Vec<u8> {
    let mut bytes = base::definition(
        &[
            ("ID", 4, fixed_offset as u16, 4),
            ("Group", 4, 4, 4),
            ("Blob", 11, 0, 0),
            ("Score", 12, 0, 0),
        ],
        count as u32,
        11 << 8,
    );
    raw::put16(&mut bytes, 43, 2);
    for (position, ordinal) in [(2, 1), (3, 0)] {
        let at = 63 + 25 * position;
        bytes[at + 15] = 2;
        raw::put16(&mut bytes, at + 7, ordinal);
    }
    // Existing physical descriptors and real logical aliases are reused whole.
    // The fourth column adds exactly25 bytes plus the10-byte Blob name record.
    bytes.copy_within(63..203, 87);
    bytes[63..87].fill(0);
    raw::put32(&mut bytes, 67, histories[0]);
    raw::put32(&mut bytes, 79, histories[1]);
    raw::put32(&mut bytes, 47, 4);
    raw::put32(&mut bytes, 51, 2);
    raw::put32(&mut bytes, 59, (11 << 8) | 1);
    bytes[227..533].copy_from_slice(&original[10 * PAGE + 192..10 * PAGE + 498]);
    for (at, number, owned, hint) in [(533, 3, 2, 3), (543, 2, 4, 5)] {
        raw::put16(&mut bytes, at, number);
        raw::put32(&mut bytes, at + 2, (11 << 8) | owned);
        raw::put32(&mut bytes, at + 6, (11 << 8) | hint);
    }
    raw::put16(&mut bytes, 553, 0xffff);
    bytes
}

fn maps(
    bytes: &mut [u8],
    data: &[u32],
    hints: &[u32],
    memo: &[u32],
    memo_hints: &[u32],
    blob: &[u32],
    blob_hints: &[u32],
) {
    frames::map_page(
        bytes,
        11,
        vec![
            frames::map(data.iter().copied()),
            frames::map(hints.iter().copied()),
            frames::map(memo.iter().copied()),
            frames::map(memo_hints.iter().copied()),
            frames::map(blob.iter().copied()),
            frames::map(blob_hints.iter().copied()),
        ],
    );
}

fn keys(rows: &[(Row, u32)], primary: bool) -> Vec<Vec<u8>> {
    let mut result = rows
        .iter()
        .map(|(row, reference)| {
            let mut key = if primary {
                [
                    vec![127],
                    ((row.id as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                ]
                .concat()
            } else {
                vec![0]
            };
            key.extend(reference.to_be_bytes());
            key
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn source_indexes(before: &mut [u8], rows: &[(Row, u32)], pressure: bool) {
    for physical in 0..2 {
        let stream = keys(rows, physical == 0);
        let seed = usize::from(pressure && physical == 0);
        let active = frames::framing(&stream, seed).1;
        let mut page = frames::leaf(None, &stream, seed, 0, 0);
        page[480 + active..].fill(if physical == 0 { 0xb6 } else { 0xc7 });
        frames::install(before, 13 + physical as u32, &page);
    }
}

fn expected_indexes(before: &[u8], after: &mut [u8], rows: &[(Row, u32)], pressure: bool) {
    let primary = keys(rows, true);
    if pressure {
        let middle = primary.len() / 2;
        assert_eq!(middle, 226);
        frames::install(after, 23, &frames::leaf(None, &primary[middle..], 0, 24, 0));
        frames::install(after, 24, &frames::leaf(None, &primary[..middle], 0, 0, 23));
        frames::install(
            after,
            13,
            &frames::root(&before[13 * PAGE..14 * PAGE], 24, 23, &primary[middle - 1]),
        );
        frames::map_page(
            after,
            15,
            vec![frames::map([13, 23, 24]), frames::map([14])],
        );
    } else {
        frames::install(
            after,
            13,
            &frames::leaf(Some(&before[13 * PAGE..14 * PAGE]), &primary, 0, 0, 0),
        );
    }
    frames::install(
        after,
        14,
        &frames::leaf(
            Some(&before[14 * PAGE..15 * PAGE]),
            &keys(rows, false),
            0,
            0,
            0,
        ),
    );
}

pub(in crate::table::mutation::insert::tests::long_value_inserts) fn source_and_expected(
    code: u8,
    pressure: bool,
) -> Case {
    let mut before = external::original(code, Family::Memo).before;
    let fixed_offset = if pressure { 0 } else { 3970 };
    let histories = if pressure { [417, 829] } else { [17, 29] };
    let count = if pressure { 400 } else { 1 };
    let metadata = definition(&before, count, fixed_offset, histories);
    frames::install(&mut before, 10, &metadata);
    before.resize(if pressure { 20 } else { 18 } * PAGE, 0);
    frames::install(&mut before, 16, &long_page(&[vec![0xd6; 4076]]));
    frames::install(&mut before, 17, &long_page(&[vec![0xd7; 4076]]));
    let ids = if pressure {
        (-200..0).chain(1..201).collect::<Vec<_>>()
    } else {
        vec![1]
    };
    let mut rows = Vec::new();
    let data = if pressure { vec![12, 18, 19] } else { vec![12] };
    let per_page = if pressure { 134 } else { 1 };
    for (number, chunk) in data.iter().zip(ids.chunks(per_page)) {
        let mut encoded = chunk
            .iter()
            .map(|id| (0, record(*id, fixed_offset, None, None, true)))
            .collect::<Vec<_>>();
        if pressure && *number == 19 {
            encoded.push((0x8000, vec![0xcc; 1300]));
        }
        frames::install(&mut before, *number, &data_page(10, &encoded));
        rows.extend(chunk.iter().enumerate().map(|(slot, id)| {
            (
                Row {
                    id: *id,
                    blob: None,
                    memo: None,
                },
                (*number << 8) | slot as u32,
            )
        }));
    }
    assert_eq!(rows.len(), count);
    maps(
        &mut before,
        &data,
        &[*data.last().unwrap()],
        &[16],
        &[16],
        &[17],
        &[17],
    );
    let eof = if pressure { 20 } else { 18 };
    frames::map_page(&mut before, 1, vec![frames::map(eof..512)]);
    source_indexes(&mut before, &rows, pressure);
    assert_eq!(
        u16::from_le_bytes(before[16 * PAGE + 2..16 * PAGE + 4].try_into().unwrap()),
        4
    );
    assert_eq!(
        u16::from_le_bytes(before[17 * PAGE + 2..17 * PAGE + 4].try_into().unwrap()),
        4
    );
    if pressure {
        assert_eq!(
            u16::from_le_bytes(before[19 * PAGE + 2..19 * PAGE + 4].try_into().unwrap()),
            8
        );
        assert_eq!(
            u16::from_le_bytes(before[13 * PAGE + 2..13 * PAGE + 4].try_into().unwrap()),
            415
        );
    }
    let added = (0..if pressure { 52 } else { 1 })
        .map(|position| Row {
            id: if pressure { 1000 + position } else { 10 },
            blob: Some(vec![0x85; if pressure { 65 } else { 64 }]),
            memo: Some("Ā".repeat(if pressure { 33 } else { 32 })),
        })
        .collect::<Vec<_>>();
    let mut expected = before.clone();
    expected.resize(if pressure { 25 } else { 20 } * PAGE, 0);
    let memo_page = if pressure { 20 } else { 18 };
    let blob_page = 21;
    let data_page_number = if pressure { 22 } else { 19 };
    let mut memo_rows = Vec::new();
    let mut blob_rows = Vec::new();
    let mut ordinary_rows = Vec::new();
    let mut final_rows = rows.clone();
    for (slot, row) in added.iter().enumerate() {
        let memo_payload = [0, 1].repeat(if pressure { 33 } else { 32 });
        memo_rows.push(memo_payload.clone());
        let memo = descriptor(&memo_payload, Some((memo_page << 8) | slot as u32));
        let blob_payload = row.blob.as_ref().unwrap();
        let blob = if pressure {
            blob_rows.push(blob_payload.clone());
            descriptor(blob_payload, Some((blob_page << 8) | slot as u32))
        } else {
            descriptor(blob_payload, None)
        };
        ordinary_rows.push((
            0,
            record(row.id, fixed_offset, Some(&blob), Some(&memo), false),
        ));
        final_rows.push((row.clone(), (data_page_number << 8) | slot as u32));
    }
    frames::install(&mut expected, memo_page, &long_page(&memo_rows));
    if pressure {
        frames::install(&mut expected, blob_page, &long_page(&blob_rows));
    }
    frames::install(
        &mut expected,
        data_page_number,
        &data_page(10, &ordinary_rows),
    );
    maps(
        &mut expected,
        &if pressure {
            vec![12, 18, 19, 22]
        } else {
            vec![12, 19]
        },
        &[data_page_number],
        &[16, memo_page],
        &[memo_page],
        &if pressure {
            vec![17, blob_page]
        } else {
            vec![17]
        },
        &if pressure { vec![blob_page] } else { vec![17] },
    );
    frames::map_page(
        &mut expected,
        1,
        vec![frames::map(if pressure { 25..512 } else { 20..512 })],
    );
    expected_indexes(&before, &mut expected, &final_rows, pressure);
    raw::put32(&mut expected, 10 * PAGE + 16, final_rows.len() as u32);
    raw::put32(
        &mut expected,
        10 * PAGE + 67,
        histories[0] + added.len() as u32,
    );
    raw::put32(&mut expected, 10 * PAGE + 79, histories[1]);
    assert_eq!(
        &expected[16 * PAGE..18 * PAGE],
        &before[16 * PAGE..18 * PAGE]
    );
    if pressure {
        assert_eq!(
            u16::from_le_bytes(expected[22 * PAGE + 2..22 * PAGE + 4].try_into().unwrap()),
            1742
        );
        assert_eq!(
            u16::from_le_bytes(expected[13 * PAGE + 2..13 * PAGE + 4].try_into().unwrap()),
            3603
        );
    } else {
        assert_eq!(ordinary_rows[0].1.len(), 4073);
        assert_eq!(
            u16::from_le_bytes(expected[19 * PAGE + 2..19 * PAGE + 4].try_into().unwrap()),
            7
        );
    }
    Case {
        before,
        rows,
        added,
        expected,
    }
}

pub(super) fn checked(bytes: &[u8], rows: &[(Row, u32)]) {
    original::checked(bytes, rows.len(), 4);
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), rows.len());
    for (actual, (row, _)) in table.rows().iter().zip(rows) {
        assert_eq!(actual, &row.values());
    }
    let long = check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(long.columns().len(), 2);
    for column in long.columns() {
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        assert_eq!(column.shared_references(), 0);
        assert_eq!(column.shared_owned_pages(), 0);
        assert_eq!(column.orphan_rows(), 1);
    }
}

pub(super) fn final_rows(case: &Case) -> Vec<(Row, u32)> {
    let data = if case.rows.len() == 400 { 22 } else { 19 };
    case.rows
        .iter()
        .cloned()
        .chain(
            case.added
                .iter()
                .cloned()
                .enumerate()
                .map(|(slot, row)| (row, (data << 8) | slot as u32)),
        )
        .collect()
}
