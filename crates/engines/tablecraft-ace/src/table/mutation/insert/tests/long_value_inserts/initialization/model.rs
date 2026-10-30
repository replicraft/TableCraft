//! Source/request-only expected bytes; no producer map renderer or LVAL encoder.
use super::super::{Family, PAGE, external, frames, original, payload, raw};
use crate::{LongValueCheckStatus, check_table_long_values};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::multiple_indexes as source_frames;

const CONTAINER: u32 = 18;
const FIRST_BODY: u32 = 19;

pub(in crate::table::mutation::insert) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub value: Value,
}

pub(super) fn values() -> Vec<(Family, Value)> {
    vec![
        (Family::Ole, Value::Binary(vec![0x92; 65])),
        (Family::Ole, Value::Binary(vec![0x92; 8145])),
        (Family::Memo, Value::Text("Ā".repeat(33))),
        (Family::Memo, Value::Text("Ā".repeat(4073))),
        (Family::CompressedMemo, Value::Text("A".repeat(65))),
    ]
}

fn long_page(body: &[u8]) -> Vec<u8> {
    let mut page = raw::data_page(0, &[(0, body.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    page[2..4].copy_from_slice(&((PAGE - 16 - body.len()) as u16).to_le_bytes());
    page
}

pub(in crate::table::mutation::insert) fn case(code: u8, family: Family, value: Value) -> Case {
    let mut source = external::original(code, family);
    let pointer = 10 * PAGE + 500;
    assert_eq!(&source.before[pointer - 2..pointer], &[2, 0]);
    assert_eq!(&source.before[pointer + 8..pointer + 10], &[255, 255]);
    source.before[pointer..pointer + 8].fill(0);
    let encoded = payload(&value, family).unwrap();
    assert!(encoded.len() > 64);
    let chained = encoded.len() > 4076;
    let chunks = if chained {
        encoded.chunks(4072).collect::<Vec<_>>()
    } else {
        vec![encoded.as_slice()]
    };
    let bodies = (FIRST_BODY..FIRST_BODY + chunks.len() as u32).collect::<Vec<_>>();
    let mut descriptor = ((if chained { 0 } else { 0x40000000 }) | encoded.len() as u32)
        .to_le_bytes()
        .to_vec();
    descriptor.extend((FIRST_BODY << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut expected = frames::expected(
        &source,
        &[source_frames::Row {
            id: 10,
            group: Some(100),
            binary: Some(descriptor),
        }],
    );
    // Existing containers and their now-unreferenced historical rows stay byte exact.
    expected[PAGE..2 * PAGE].copy_from_slice(&source.before[PAGE..2 * PAGE]);
    expected[11 * PAGE..12 * PAGE].copy_from_slice(&source.before[11 * PAGE..12 * PAGE]);
    expected.resize((FIRST_BODY as usize + chunks.len()) * PAGE, 0);
    expected[pointer..pointer + 4].copy_from_slice(&(CONTAINER << 8).to_le_bytes());
    expected[pointer + 4..pointer + 8].copy_from_slice(&((CONTAINER << 8) | 1).to_le_bytes());
    let records = [
        (0, source_frames::map(bodies.iter().copied())),
        (0, source_frames::map([*bodies.last().unwrap()])),
    ];
    let mut container = raw::data_page(0, &records);
    container[2..4].copy_from_slice(&3940_u16.to_le_bytes());
    assert_eq!(&container[..4], &[1, 1, 100, 15]);
    source_frames::install(&mut expected, CONTAINER, &container);
    for (position, part) in chunks.iter().enumerate() {
        let mut body = Vec::new();
        if chained {
            body.extend(
                bodies
                    .get(position + 1)
                    .map_or(0, |number| number << 8)
                    .to_le_bytes(),
            );
        }
        body.extend_from_slice(part);
        source_frames::install(&mut expected, bodies[position], &long_page(&body));
    }
    // GLOBAL row0 has base zero; clear only actual new container/body cells.
    let row = u16::from_le_bytes(expected[PAGE + 14..PAGE + 16].try_into().unwrap()) as usize;
    for number in CONTAINER..FIRST_BODY + chunks.len() as u32 {
        expected[PAGE + row + 5 + number as usize / 8] &= !(1 << (number % 8));
    }
    Case {
        before: source.before,
        expected,
        value,
    }
}

pub(in crate::table::mutation::insert) fn rows(
    family: Family,
    added: Option<&Value>,
) -> Vec<Vec<Value>> {
    let old = if matches!(family, Family::Ole) {
        vec![
            Value::Null,
            Value::Binary(vec![]),
            Value::Binary(vec![0, 128, 255]),
        ]
    } else {
        vec![
            Value::Null,
            Value::Text(String::new()),
            Value::Text("ABC".into()),
        ]
    };
    let mut rows = old
        .into_iter()
        .enumerate()
        .map(|(position, value)| {
            let id = position as i32 + 1;
            vec![Value::Int32(id), Value::Int32(id * 10), value]
        })
        .collect::<Vec<_>>();
    if let Some(value) = added {
        rows.push(vec![Value::Int32(10), Value::Int32(100), value.clone()]);
    }
    rows
}

pub(in crate::table::mutation::insert) fn checked_maps(bytes: &[u8]) {
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
