#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded;
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ignored_null_indexes/fixture.rs"]
mod ignored;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
mod support;

use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, NativeImage, RowDelete, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 30_000_000)
}

fn raw(row: usize, value: Option<i32>) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend((row as i32 + 1).to_le_bytes());
    bytes.extend(value.unwrap_or(0).to_le_bytes());
    bytes.push(if value.is_some() { 3 } else { 1 });
    bytes
}

#[test]
fn nullable_single_keys_keep_exact_effective_removals_and_zero_effective_bytes() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for values in [vec![None, Some(10), None, Some(20)], vec![None, None]] {
                    let bytes = ignored::database(code, descending, unique, false, &values);
                    let choices = if values.len() == 2 {
                        vec![vec![0]]
                    } else {
                        vec![vec![0, 2], vec![0, 1], vec![1, 3]]
                    };
                    for deleted in choices {
                        let image = NativeImage::capture(
                            &mut Cursor::new(&bytes),
                            CatalogScope::new(1),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let requests = deleted
                            .iter()
                            .map(|&row| RowDelete::new(row, raw(row, values[row])))
                            .collect::<Vec<_>>();
                        let output = image
                            .preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
                            .unwrap();
                        let after = output.candidate_bytes();
                        assert_eq!(output.deleted_rows(), deleted.len());
                        assert_eq!(
                            &after[10 * 4096 + 67..10 * 4096 + 71],
                            &bytes[10 * 4096 + 67..10 * 4096 + 71]
                        );
                        if deleted.iter().all(|&row| values[row].is_none()) {
                            assert_eq!(&after[13 * 4096..14 * 4096], &bytes[13 * 4096..14 * 4096]);
                        }
                        let expected = values
                            .iter()
                            .enumerate()
                            .filter(|(row, _)| !deleted.contains(row))
                            .map(|(row, value)| {
                                vec![
                                    Value::Int32(row as i32 + 1),
                                    value.map_or(Value::Null, Value::Int32),
                                ]
                            })
                            .collect::<Vec<_>>();
                        let table = read_table(
                            &mut Cursor::new(after),
                            CatalogScope::new(1),
                            "Items",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(table.rows(), expected);
                        let checked = check_table(
                            &mut Cursor::new(after),
                            CatalogScope::new(1),
                            "Items",
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        assert!(
                            checked
                                .indexes()
                                .iter()
                                .all(|index| index.status() == IndexCheckStatus::Checked)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn ignored_preserved_text_and_binary_distinguish_null_from_empty_values() {
    for descending in [false, true] {
        for mut model in [
            encoded::text(0, descending, true),
            encoded::text(1, descending, true),
            encoded::binary(descending, true),
        ] {
            model.bytes[10 * 4096 + 189] = 0x82;
            let entries = model
                .order
                .iter()
                .copied()
                .filter(|&row| row != 0)
                .map(|row| encoded::wire(&model, row))
                .collect::<Vec<_>>();
            model.bytes[13 * 4096..14 * 4096].copy_from_slice(&encoded::page(0, &entries, 0, 0, 0));
            for deleted in [vec![0], vec![1], vec![0, 1]] {
                let output = encoded::candidate(&model, &deleted).unwrap();
                if deleted == [0] {
                    assert_eq!(
                        &output.candidate_bytes()[13 * 4096..14 * 4096],
                        &model.bytes[13 * 4096..14 * 4096]
                    );
                }
                let expected = model
                    .order
                    .iter()
                    .copied()
                    .filter(|row| *row != 0 && !deleted.contains(row))
                    .map(|row| encoded::wire(&model, row))
                    .collect::<Vec<_>>();
                assert_eq!(encoded::entries(output.candidate_bytes(), 13), expected);
                assert_eq!(output.deleted_rows(), deleted.len());
            }
        }
    }
}
