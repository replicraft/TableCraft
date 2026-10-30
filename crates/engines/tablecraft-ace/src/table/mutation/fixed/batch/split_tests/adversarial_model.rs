//! Additional independent literal sources; the frozen eighty-case model is unchanged.
use super::model::{self, Case, PAGE, Row, Shape};
use crate::FixedScalarUpdate;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::multiple_indexes as frames;

pub(super) fn assignments(case: &Case, right: bool) -> Vec<FixedScalarUpdate> {
    case.original
        .iter()
        .zip(&case.final_rows)
        .enumerate()
        .map(|(position, (old, new))| {
            let before = if right { old.right } else { old.left };
            let after = if right { new.right } else { new.left };
            FixedScalarUpdate::new(
                position,
                before.map_or(Value::Null, Value::Int32),
                after.map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

pub(super) struct DuplicateFamily {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: Vec<Row>,
    pub rows: Vec<Row>,
}

pub(super) fn duplicate_family(code: u8, descending: bool) -> DuplicateFamily {
    let case = model::source(code, Shape::Empty, descending, false, false);
    let mut rows = case.original.clone();
    let mut expected = case.before.clone();
    for row in &mut rows {
        row.left = Some(7);
        row.right = Some(9);
        let range = model::span(&case.before, row.reference);
        expected[range.start + 6..range.start + 10].copy_from_slice(&7_i32.to_le_bytes());
        expected[range.start + 10..range.start + 14].copy_from_slice(&9_i32.to_le_bytes());
        expected[range.end - 1] = 7;
    }
    let left = if descending {
        vec![0x80, 0x7f, 0xff, 0xff, 0xf8]
    } else {
        vec![0x7f, 0x80, 0, 0, 7]
    };
    let mut pair = if descending {
        vec![0x7f, 0x80, 0, 0, 9]
    } else {
        vec![0x80, 0x7f, 0xff, 0xff, 0xf6]
    };
    pair.extend_from_slice(&left);
    for (page, key) in [(13_u32, left), (14, pair)] {
        let entries = rows
            .iter()
            .map(|row| {
                let mut wire = key.clone();
                wire.extend(row.reference.to_be_bytes());
                wire
            })
            .collect::<Vec<_>>();
        let original = &case.before[page as usize * PAGE..(page as usize + 1) * PAGE];
        frames::install(
            &mut expected,
            page,
            &frames::leaf(Some(original), &entries, 0, 0, 0),
        );
    }
    // Five hundred newly eligible rows contribute one new family, once across aliases.
    raw::put32(&mut expected, 10 * PAGE + 79, 801);
    DuplicateFamily {
        before: case.before,
        expected,
        original: case.original,
        rows,
    }
}

fn append_catalog(bytes: &mut [u8], record: Vec<u8>) {
    let count = usize::from(u16::from_le_bytes(
        bytes[4 * PAGE + 12..4 * PAGE + 14].try_into().unwrap(),
    ));
    let mut rows = (0..count)
        .map(|slot| {
            (
                0,
                bytes[model::span(bytes, (4 << 8) | slot as u32)].to_vec(),
            )
        })
        .collect::<Vec<_>>();
    rows.push((0, record));
    raw::put32(bytes, 2 * PAGE + 16, rows.len() as u32);
    frames::install(bytes, 4, &raw::data_page(2, &rows));
}

pub(super) fn table_alias(code: u8) -> Vec<u8> {
    let mut bytes = model::source(code, Shape::Mixed, false, false, false).before;
    append_catalog(
        &mut bytes,
        raw::record(0x0100_000a, 0, 1, 0, b"\xff\xfeAlias"),
    );
    bytes
}

pub(super) fn two_tables(code: u8) -> Vec<u8> {
    let mut bytes = model::source(code, Shape::Mixed, false, false, false).before;
    bytes.resize(21 * PAGE, 0);
    append_catalog(&mut bytes, raw::record(18, 0, 1, 0, b"\xff\xfeOther"));
    frames::install(
        &mut bytes,
        18,
        &tablecraft_testkit::access::mutation_fixture::definition(
            &[("Id", 4, 0, 4), ("Score", 4, 4, 4)],
            3,
            19 << 8,
        ),
    );
    raw::put32(&mut bytes, 18 * PAGE + 59, (19 << 8) | 1);
    frames::map_page(&mut bytes, 19, vec![frames::map([20]), frames::map([20])]);
    let rows = (1_i32..=3)
        .map(|id| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend((id * 100).to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    frames::install(&mut bytes, 20, &raw::data_page(18, &rows));
    frames::map_page(&mut bytes, 1, vec![frames::map(21..512)]);
    bytes
}
