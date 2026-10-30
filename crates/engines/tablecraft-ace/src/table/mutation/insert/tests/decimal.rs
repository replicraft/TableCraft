//! Exact NUMERIC INSERT images derived independently through literal row wires.
use super::model;
use crate::{EditError, RowInsert, WriteBlocker, read_table};
use std::io::Cursor;
use tablecraft_core::values::{Currency, Decimal, Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::insert_seed as source_model;

const PAGE: usize = 4096;
const SCORE: usize = 10 * PAGE + 100;

fn wire(coefficient: i128) -> Vec<u8> {
    let mut bytes = vec![0; 17];
    bytes[0] = if coefficient < 0 { 128 } else { 0 };
    let magnitude = coefficient.unsigned_abs();
    for position in 0..4 {
        let shift = 32 * (3 - position);
        let word = ((magnitude >> shift) & 0xffff_ffff) as u32;
        bytes[1 + 4 * position..5 + 4 * position].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn source(code: u8, alias: bool, full: bool) -> Vec<u8> {
    let mut bytes = source_model::source(code, alias, full, None);
    bytes[SCORE] = 16;
    bytes[SCORE + 11] = 28;
    bytes[SCORE + 12] = 4;
    raw::put16(&mut bytes, SCORE + 23, 17);

    let mut records = Vec::new();
    for (id, value) in [(1_i32, None), (2, Some(0)), (3, Some(-12345))] {
        let mut row = vec![2, 0];
        row.extend_from_slice(&id.to_le_bytes());
        if let Some(value) = value {
            row.extend_from_slice(&wire(value));
        }
        let end = row.len() as u16;
        row.extend_from_slice(&end.to_le_bytes());
        row.extend_from_slice(&[6, 0, 1, 0, if value.is_some() { 3 } else { 1 }]);
        records.push((0, row));
    }
    if full {
        records.push((0x8000, vec![0x9d; 3800]));
    }
    let remaining =
        PAGE - 14 - records.len() * 2 - records.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut page = raw::data_page(10, &records);
    raw::put16(&mut page, 2, remaining as u16);
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&page);
    bytes
}

fn value(coefficient: Option<i128>) -> Value {
    coefficient.map_or(Value::Null, |value| {
        Value::Decimal(Decimal::new(value, 4).unwrap())
    })
}

#[test]
fn decimal_inserts_keep_exact_scale_sign_word_order_nulls_and_complete_images() {
    assert_eq!(
        wire(311917102708983781990730508),
        [0, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9],
    );
    for code in [5, 6] {
        for alias in [false, true] {
            for full in [false, true] {
                let before = source(code, alias, full);
                model::checked(&before, 3, if alias { 2 } else { 1 });
                let coefficients = [None, Some(0), Some(311917102708983781990730508), Some(-1)];
                let raw_requests = coefficients
                    .iter()
                    .enumerate()
                    .map(|(position, coefficient)| model::NewRow {
                        id: 10 + position as i32,
                        binary: coefficient.map(wire),
                    })
                    .collect::<Vec<_>>();
                let (expected, _) = model::expected(&before, &raw_requests);
                model::checked(&expected, 7, if alias { 2 } else { 1 });

                let requests = coefficients
                    .iter()
                    .enumerate()
                    .map(|(position, coefficient)| {
                        RowInsert::new(vec![
                            Value::Int32(10 + position as i32),
                            value(*coefficient),
                        ])
                    })
                    .collect::<Vec<_>>();
                let candidate = model::capture(&before)
                    .preview_ace16_insert_rows("Items", &requests, &mut model::budget(), &|| false)
                    .unwrap();
                assert_eq!(candidate.candidate_bytes(), expected);
                assert_eq!(candidate.changed_rows(), 4);
                let table = read_table(
                    &mut Cursor::new(candidate.candidate_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                for request in &requests {
                    assert!(table.rows().iter().any(|row| row == request.values()));
                }
            }
        }
    }
}

#[test]
fn decimal_inserts_do_not_rescale_round_or_coerce_and_reject_unqualified_declarations() {
    let before = source(6, false, false);
    for invalid in [
        Value::Int32(1),
        Value::Currency(Currency::from_scaled(1)),
        Value::Decimal(Decimal::new(1, 3).unwrap()),
        Value::Decimal(Decimal::new(10_i128.pow(28), 4).unwrap()),
    ] {
        let result = model::capture(&before).preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(vec![Value::Int32(10), invalid])],
            &mut model::budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
    }
    for (offset, invalid) in [(11, 0), (12, 29), (15, 3), (16, 1)] {
        let mut bad = before.clone();
        bad[SCORE + offset] = invalid;
        let result = model::capture(&bad).preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(vec![Value::Int32(10), Value::Null])],
            &mut model::budget(),
            &|| false,
        );
        assert!(result.is_err());
    }
}
