//! Literal map rows, chunk pages and complete images fixed before preview calls.
mod indirect;

use super::super::{Family, PAGE, external, frames, original};
use crate::{LongValueCheckStatus, check_table_long_values};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub(in crate::table::mutation::insert) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub value: Vec<u8>,
    pub bodies: Vec<u32>,
    pub bitmaps: Vec<u32>,
    pub empty: bool,
}

fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * PAGE + 14 + slot * 2;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    page * PAGE + usize::from(word & 8191)
}

fn inline_map(base: u32, members: &[u32]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    bytes[1..5].copy_from_slice(&base.to_le_bytes());
    for &number in members {
        let relative = number.checked_sub(base).unwrap();
        assert!(relative < 512);
        bytes[5 + relative as usize / 8] |= 1 << (relative % 8);
    }
    bytes
}

fn map_row(bytes: &mut [u8], slot: usize, record: &[u8]) {
    assert_eq!(record.len(), 69);
    let start = row_start(bytes, 11, slot);
    bytes[start..start + 69].copy_from_slice(record);
}

fn lval_page(body: &[u8]) -> Vec<u8> {
    assert!(!body.is_empty() && body.len() <= 4076);
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    page[4..8].copy_from_slice(b"LVAL");
    page[12..14].copy_from_slice(&1_u16.to_le_bytes());
    let start = PAGE - body.len();
    page[14..16].copy_from_slice(&(start as u16).to_le_bytes());
    page[2..4].copy_from_slice(&((start - 16) as u16).to_le_bytes());
    page[start..].copy_from_slice(body);
    page
}

fn source(code: u8, eof: u32, empty: bool) -> frames::Case {
    let mut case = external::original(code, Family::Ole);
    case.before.resize(eof as usize * PAGE, 0);
    case.before[16 * PAGE..17 * PAGE].copy_from_slice(&lval_page(&[0x5a; 4076]));
    if empty {
        map_row(&mut case.before, 2, &inline_map(0, &[]));
        map_row(&mut case.before, 3, &inline_map(0, &[]));
    }

    let start = row_start(&case.before, 1, 0);
    let mut global = vec![0; 69];
    global[0] = 1;
    global[1..5].copy_from_slice(&17_u32.to_le_bytes());
    case.before[start..start + 69].copy_from_slice(&global);
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in eof..32736 {
        bitmap[4 + number as usize / 8] |= 1 << (number % 8);
    }
    case.before[17 * PAGE..18 * PAGE].copy_from_slice(&bitmap);
    case
}

pub(in crate::table::mutation::insert) fn source_and_expected(
    code: u8,
    eof: u32,
    length: usize,
    empty: bool,
) -> Case {
    let case = source(code, eof, empty);
    let value = (0..length)
        .map(|position| (position * 73 + 19) as u8)
        .collect::<Vec<_>>();
    let chained = length > 4076;
    let promoted = !empty && eof == 528;
    assert!(matches!(
        (eof, length, empty),
        (527, 65, false) | (528, 65, false) | (528, 8145, false) | (600, 65, true)
    ));
    let bodies = if chained {
        vec![528, 530, 531]
    } else {
        vec![eof]
    };
    let bitmaps = if promoted { vec![529] } else { vec![] };
    let tag = if chained { 0 } else { 0x40000000 };
    let mut descriptor = (tag | length as u32).to_le_bytes().to_vec();
    descriptor.extend((eof << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let added = frames::Row {
        id: 10,
        group: Some(100),
        binary: Some(descriptor),
    };
    let mut expected = frames::expected(&case, &[added]);
    // The old multi-index model owns DATA/key/counter bytes. These inputs fit
    // existing DATA12 and both roots, so their physical map memberships do not
    // change. Retain the actual four-row column-map container and GLOBAL map.
    expected[PAGE..2 * PAGE].copy_from_slice(&case.before[PAGE..2 * PAGE]);
    expected[11 * PAGE..12 * PAGE].copy_from_slice(&case.before[11 * PAGE..12 * PAGE]);
    let end = bodies.iter().chain(&bitmaps).max().unwrap() + 1;
    expected.resize(end as usize * PAGE, 0);
    if chained {
        for (ordinal, part) in value.chunks(4072).enumerate() {
            let mut body = bodies
                .get(ordinal + 1)
                .map_or(0, |number| number << 8)
                .to_le_bytes()
                .to_vec();
            body.extend(part);
            let page = bodies[ordinal] as usize;
            expected[page * PAGE..(page + 1) * PAGE].copy_from_slice(&lval_page(&body));
        }
    } else {
        expected[eof as usize * PAGE..(eof as usize + 1) * PAGE]
            .copy_from_slice(&lval_page(&value));
    }
    let members = if empty {
        bodies.clone()
    } else {
        std::iter::once(16)
            .chain(bodies.iter().copied())
            .collect::<Vec<_>>()
    };
    if promoted {
        let mut owned = vec![0; 69];
        owned[0] = 1;
        owned[1..5].copy_from_slice(&529_u32.to_le_bytes());
        map_row(&mut expected, 2, &owned);
        let mut bitmap = vec![0; PAGE];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        for &number in &members {
            bitmap[4 + number as usize / 8] |= 1 << (number % 8);
        }
        expected[529 * PAGE..530 * PAGE].copy_from_slice(&bitmap);
    } else {
        map_row(
            &mut expected,
            2,
            &inline_map(if empty { 600 } else { 16 }, &members),
        );
    }
    let last = *bodies.last().unwrap();
    map_row(&mut expected, 3, &inline_map(last - last % 8, &[last]));
    for &number in bodies.iter().chain(&bitmaps) {
        expected[17 * PAGE + 4 + number as usize / 8] &= !(1 << (number % 8));
    }
    assert_eq!(
        &expected[16 * PAGE..17 * PAGE],
        &case.before[16 * PAGE..17 * PAGE]
    );
    Case {
        before: case.before,
        expected,
        value,
        bodies,
        bitmaps,
        empty,
    }
}

pub(in crate::table::mutation::insert) fn rows(added: Option<&[u8]>) -> Vec<Vec<Value>> {
    let mut rows = vec![
        vec![Value::Int32(1), Value::Int32(10), Value::Null],
        vec![Value::Int32(2), Value::Int32(20), Value::Binary(vec![])],
        vec![
            Value::Int32(3),
            Value::Int32(30),
            Value::Binary(vec![0, 128, 255]),
        ],
    ];
    if let Some(value) = added {
        rows.push(vec![
            Value::Int32(10),
            Value::Int32(100),
            Value::Binary(value.to_vec()),
        ]);
    }
    rows
}

pub(in crate::table::mutation::insert) fn checked(bytes: &[u8], rows: &[Vec<Value>]) {
    super::super::checked(bytes, rows);
    let report = check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Checked);
    assert_eq!(report.columns()[0].shared_references(), 0);
    assert_eq!(report.columns()[0].shared_owned_pages(), 0);
}
