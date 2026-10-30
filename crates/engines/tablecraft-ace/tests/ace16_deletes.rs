#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, NativeImage, RowDelete, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 20_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn raw_record(id: i32, kind: u8, value: Option<i64>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    let stored = value.unwrap_or(0);
    match kind {
        1 => {}
        2 => bytes.push(stored as u8),
        3 => bytes.extend((stored as i16).to_le_bytes()),
        4 | 6 => bytes.extend((stored as i32).to_le_bytes()),
        5 | 7 | 8 | 0x13 => bytes.extend(stored.to_le_bytes()),
        _ => panic!("unsupported authored field kind"),
    }
    let present = if kind == 1 {
        stored == -1
    } else {
        value.is_some()
    };
    bytes.push(if present { 3 } else { 1 });
    bytes
}

#[test]
fn root_leaf_deletion_reuses_every_numeric_writer_codec_including_float_and_date() {
    let cases = [
        (1, Some(0), Some(-1)),
        (2, Some(0), Some(255)),
        (3, Some(-32768), Some(32767)),
        (4, Some(-2147483648), Some(2147483647)),
        (0x13, Some(i64::MIN), Some(i64::MAX)),
        (5, Some(i64::MIN), Some(i64::MAX)),
        (
            6,
            Some(i64::from((-0.0_f32).to_bits())),
            Some(i64::from(1.0_f32.to_bits())),
        ),
        (
            7,
            Some((-0.0_f64).to_bits() as i64),
            Some(1.0_f64.to_bits() as i64),
        ),
        (
            8,
            Some(0.0_f64.to_bits() as i64),
            Some(1.0_f64.to_bits() as i64),
        ),
    ];
    for code in [5, 6] {
        for descending in [false, true] {
            for (kind, first, second) in cases {
                let third = if kind == 1 { Some(0) } else { None };
                let values = [first, second, third];
                let rows = values
                    .iter()
                    .enumerate()
                    .map(|(row, value)| vec![Some(row as i64 + 1), *value])
                    .collect::<Vec<_>>();
                let bytes =
                    numeric_fixture::database(code, &[4, kind], &[(1, descending)], &rows, 0);
                for removed in [vec![1], vec![0, 1, 2]] {
                    let requests = removed
                        .iter()
                        .map(|&row| {
                            RowDelete::new(row, raw_record(row as i32 + 1, kind, values[row]))
                        })
                        .collect::<Vec<_>>();
                    let candidate = capture(&bytes)
                        .preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(candidate.before_bytes(), bytes);
                    assert_eq!(candidate.deleted_rows(), removed.len());
                    assert_eq!(candidate.candidate_bytes().len(), bytes.len());
                    let report = check_table(
                        &mut Cursor::new(candidate.candidate_bytes()),
                        CatalogScope::new(1),
                        "Items",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert!(
                        report
                            .indexes()
                            .iter()
                            .all(|index| index.status() == IndexCheckStatus::Checked)
                    );
                    let result = read_table(
                        &mut Cursor::new(candidate.candidate_bytes()),
                        CatalogScope::new(1),
                        "Items",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let ids = result
                        .rows()
                        .iter()
                        .map(|row| row[0].clone())
                        .collect::<Vec<_>>();
                    let expected = (0..3)
                        .filter(|row| !removed.contains(row))
                        .map(|row| Value::Int32(row as i32 + 1))
                        .collect::<Vec<_>>();
                    assert_eq!(ids, expected, "kind={kind}, descending={descending}");
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn arbitrary_integral_keys_and_delete_sets_preserve_exact_remaining_rows(
        keys in prop::array::uniform3(any::<i32>()),
        remove in prop::array::uniform3(any::<bool>()),
    ) {
        let rows = keys
            .iter()
            .enumerate()
            .map(|(row, key)| {
                vec![Some(row as i64 + 1), Some(i64::from(*key))]
            })
            .collect::<Vec<_>>();
        let bytes = numeric_fixture::database(
            6,
            &[4, 4],
            &[(1, false)],
            &rows,
            0,
        );
        let requests = remove
            .iter()
            .enumerate()
            .filter(|(_, selected)| **selected)
            .map(|(row, _)| {
                RowDelete::new(
                    row,
                    raw_record(row as i32 + 1, 4, Some(i64::from(keys[row]))),
                )
            })
            .collect::<Vec<_>>();
        let candidate = capture(&bytes)
            .preview_ace16_deletes(
                "Items",
                &requests,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let result = read_table(
            &mut Cursor::new(candidate.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = keys
            .iter()
            .enumerate()
            .filter(|(row, _)| !remove[*row])
            .map(|(row, key)| {
                vec![Value::Int32(row as i32 + 1), Value::Int32(*key)]
            })
            .collect::<Vec<_>>();
        prop_assert_eq!(result.rows(), expected.as_slice());
        prop_assert_eq!(candidate.deleted_rows(), requests.len());
    }
}
