#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod base;
#[path = "ace16_delete_trees/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, NativeImage, RowDelete, WriteBlocker, check_table,
    read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const ENTRY_START: usize = 480;
const ROOT_PAGE: usize = 14;

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

#[test]
fn every_numeric_codec_preserves_leaf_partitions_and_exact_node_maxima() {
    for code in [5, 6] {
        for kind in [1, 2, 3, 4, 0x13, 5, 6, 7, 8] {
            for descending in [false, true] {
                for bounded in [false, true] {
                    let (bytes, partitions) = fixture::database(code, kind, descending, bounded);
                    let removed = partitions
                        .iter()
                        .map(|rows| *rows.last().unwrap())
                        .collect::<Vec<_>>();
                    let requests = removed
                        .iter()
                        .map(|&row| RowDelete::new(row, fixture::raw_record(&bytes, row)))
                        .collect::<Vec<_>>();
                    let candidate = capture(&bytes)
                        .preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
                        .unwrap();
                    let after = candidate.candidate_bytes();
                    assert_eq!(candidate.deleted_rows(), 3);
                    assert_eq!(candidate.before_bytes(), bytes);
                    assert_eq!(after.len(), bytes.len());
                    let expected = partitions
                        .iter()
                        .flatten()
                        .filter(|row| !removed.contains(row))
                        .map(|row| Value::Int32(*row as i32 + 1))
                        .collect::<Vec<_>>();
                    let read = read_table_index(
                        &mut Cursor::new(after),
                        CatalogScope::new(1),
                        "Items",
                        "Compound",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        read.rows()
                            .iter()
                            .map(|row| row[0].clone())
                            .collect::<Vec<_>>(),
                        expected
                    );
                    let values = fixture::values(kind);
                    let mut separators = Vec::new();
                    for (position, rows) in partitions.iter().enumerate() {
                        if !bounded && position == partitions.len() - 1 {
                            break;
                        }
                        let maximum = rows
                            .iter()
                            .rev()
                            .find(|row| !removed.contains(row))
                            .unwrap();
                        let mut entry =
                            fixture::entry(kind, values[*maximum], *maximum, descending);
                        entry.extend((15 + position as u32).to_be_bytes());
                        separators.extend(entry);
                    }
                    let start = ROOT_PAGE * PAGE_BYTES + ENTRY_START;
                    assert_eq!(&after[start..start + separators.len()], separators);
                    for page in [ROOT_PAGE, 15, 16, 17] {
                        assert_eq!(
                            &after[page * PAGE_BYTES + 4..page * PAGE_BYTES + 24],
                            &bytes[page * PAGE_BYTES + 4..page * PAGE_BYTES + 24]
                        );
                        assert_eq!(after[page * PAGE_BYTES + 26], bytes[page * PAGE_BYTES + 26]);
                    }
                    assert_eq!(
                        &after[13 * PAGE_BYTES..14 * PAGE_BYTES],
                        &bytes[13 * PAGE_BYTES..14 * PAGE_BYTES]
                    );
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

#[test]
fn original_leaf_exhaustion_is_atomic_and_node_noops_are_byte_identical() {
    for bounded in [false, true] {
        let (bytes, partitions) = fixture::database(6, 4, false, bounded);
        let noop = capture(&bytes)
            .preview_ace16_deletes("Items", &[], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(noop.candidate_bytes(), bytes);
        let requests = partitions[0]
            .iter()
            .map(|&row| RowDelete::new(row, fixture::raw_record(&bytes, row)))
            .collect::<Vec<_>>();
        assert_eq!(
            capture(&bytes)
                .preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
}

#[test]
fn separator_growth_requiring_a_split_refuses_with_every_leaf_still_nonempty() {
    let bytes = fixture::capacity_database();
    let original = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        original
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let request = RowDelete::new(1, fixture::raw_record(&bytes, 1));
    assert_eq!(
        capture(&bytes)
            .preview_ace16_deletes("Items", &[request], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    let noop = capture(&bytes)
        .preview_ace16_deletes("Items", &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(noop.candidate_bytes(), bytes);
}
