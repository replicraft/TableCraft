//! Literal inline descriptors and whole images, independent of payload encoding.
use super::{model as original, multiple_indexes::model as frames};
use crate::{RowInsert, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;

mod composition;
pub(in crate::table::mutation::insert) mod external;

#[derive(Clone, Copy, Debug)]
pub(in crate::table::mutation::insert) enum Family {
    Ole,
    Memo,
    CompressedMemo,
}

fn payload(value: &Value, family: Family) -> Option<Vec<u8>> {
    match (value, family) {
        (Value::Null, _) => None,
        (Value::Binary(bytes), Family::Ole) => Some(bytes.clone()),
        (Value::Text(text), Family::Memo | Family::CompressedMemo) => {
            let units = text.encode_utf16().collect::<Vec<_>>();
            let compressed = matches!(family, Family::CompressedMemo)
                && (3..=1024).contains(&units.len())
                && units.iter().all(|unit| (1..=255).contains(unit));
            let bytes = if compressed {
                let mut bytes = vec![255, 254];
                bytes.extend(units.iter().map(|unit| *unit as u8));
                bytes
            } else {
                units.into_iter().flat_map(u16::to_le_bytes).collect()
            };
            Some(bytes)
        }
        _ => panic!("test supplies the declared exact carrier"),
    }
}

fn record(id: i32, value: &Value, family: Family) -> frames::Row {
    let binary = payload(value, family).map(|payload| {
        assert!(payload.len() <= 64);
        let mut descriptor = (0x80000000_u32 | payload.len() as u32)
            .to_le_bytes()
            .to_vec();
        descriptor.extend([0; 8]);
        descriptor.extend(payload);
        descriptor
    });
    frames::Row {
        id,
        group: Some(id * 10),
        binary,
    }
}

fn source(code: u8, family: Family, values: &[Value]) -> frames::Case {
    let mut case = frames::source(code, false, false);
    let column = 10 * PAGE + 137;
    case.before[column] = if matches!(family, Family::Ole) {
        11
    } else {
        12
    };
    raw::put16(&mut case.before, column + 23, 0);
    case.before[column + 16] = u8::from(matches!(family, Family::CompressedMemo));
    if !matches!(family, Family::Ole) {
        super::properties::allow_empty_text(&mut case.before);
    }
    case.rows = values
        .iter()
        .enumerate()
        .map(|(slot, value)| {
            (
                record(slot as i32 + 1, value, family),
                (12 << 8) | slot as u32,
            )
        })
        .collect();
    let records = case
        .rows
        .iter()
        .map(|(row, _)| (0, frames::record(row)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &records);
    let free =
        PAGE - 14 - 2 * records.len() - records.iter().map(|(_, row)| row.len()).sum::<usize>();
    raw::put16(&mut page, 2, free as u16);
    frames::install(&mut case.before, 12, &page);

    // ID1/2/3 remains unchanged; Group is explicitly10/20/30 in this source.
    let entries = case
        .rows
        .iter()
        .map(|(row, reference)| {
            let mut entry = vec![127];
            entry.extend(((row.group.unwrap() as u32) ^ 0x80000000).to_be_bytes());
            entry.extend(reference.to_be_bytes());
            entry
        })
        .collect::<Vec<_>>();
    frames::install(&mut case.before, 14, &frames::leaf(None, &entries, 0, 0, 0));
    case
}

fn checked(bytes: &[u8], expected: &[Vec<Value>]) {
    original::checked(bytes, expected.len(), 4);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), expected);
}

fn run(code: u8, family: Family, old: &[Value], added: &[Value]) {
    assert_eq!(old.len(), 3);
    let case = source(code, family, old);
    let logical =
        |id: i32, value: &Value| vec![Value::Int32(id), Value::Int32(id * 10), value.clone()];
    let old_rows = old
        .iter()
        .enumerate()
        .map(|(position, value)| logical(position as i32 + 1, value))
        .collect::<Vec<_>>();
    checked(&case.before, &old_rows);
    let raw_added = added
        .iter()
        .enumerate()
        .map(|(position, value)| record(position as i32 + 10, value, family))
        .collect::<Vec<_>>();
    let expected = frames::expected(&case, &raw_added);
    let rows = old_rows
        .into_iter()
        .chain(
            added
                .iter()
                .enumerate()
                .map(|(position, value)| logical(position as i32 + 10, value)),
        )
        .collect::<Vec<_>>();
    checked(&expected, &rows);
    let requests = added
        .iter()
        .enumerate()
        .map(|(position, value)| RowInsert::new(logical(position as i32 + 10, value)))
        .collect::<Vec<_>>();
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap_or_else(|error| panic!("{family:?}: {error:?}"));
    let actual = candidate.candidate_bytes();
    assert_eq!(actual.len(), expected.len());
    let difference = actual
        .iter()
        .zip(&expected)
        .position(|(actual, expected)| actual != expected);
    assert!(
        difference.is_none(),
        "first inline LVAL image difference: {difference:?}"
    );
    checked(actual, &rows);
}

#[test]
fn inline_ole_null_empty_opaque_and_64_byte_payloads_preserve_complete_images() {
    for code in [5, 6] {
        run(
            code,
            Family::Ole,
            &[
                Value::Null,
                Value::Binary(vec![]),
                Value::Binary(vec![0, 128, 255]),
            ],
            &[
                Value::Null,
                Value::Binary(vec![]),
                Value::Binary((0..64).collect()),
            ],
        );
    }
}

#[test]
fn inline_memo_retains_utf16_compression_and_the_explicit_empty_policy() {
    for code in [5, 6] {
        for family in [Family::Memo, Family::CompressedMemo] {
            let width = if matches!(family, Family::CompressedMemo) {
                62
            } else {
                32
            };
            run(
                code,
                family,
                &[
                    Value::Null,
                    Value::Text(String::new()),
                    Value::Text("ABC".into()),
                ],
                &[
                    Value::Null,
                    Value::Text(String::new()),
                    Value::Text("AB".into()),
                    Value::Text("ABC".into()),
                    Value::Text("A\0B".into()),
                    Value::Text("λ😀".into()),
                    Value::Text("a".repeat(width)),
                ],
            );
        }
    }
}
