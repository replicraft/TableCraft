use super::{open, request};
use crate::{CellKind, Command, Error, Response, Worker};
use std::path::PathBuf;
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{
    self as raw, attachment_modern, calculated_storage, complex_links, version_history,
};

const PAGE: usize = 4096;

fn opened(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, Worker) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stored.accdb");
    std::fs::write(&path, bytes).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();
    (directory, path, worker)
}

fn page(worker: &Worker, offset: u32) -> Result<Response, Error> {
    request(
        worker,
        Command::Page {
            table: "Items".to_owned(),
            offset,
        },
    )
}

#[test]
fn cached_values_and_null_keep_stored_provenance_without_evaluating_formula() {
    let bytes = calculated_storage::database_with_expression(&[Some(77), None], "1/0");
    let (_directory, path, worker) = opened(&bytes);
    let Response::Data { rows, has_more, .. } = page(&worker, 0).unwrap() else {
        panic!("expected stored values");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1].text, "77");
    assert_eq!(
        rows[0][1].kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: true
        }
    );
    assert_eq!(rows[1][1].text, "NULL");
    assert_eq!(
        rows[1][1].kind,
        CellKind::Scalar {
            is_null: true,
            stored_calculation: true
        }
    );
    assert_eq!(
        rows[0][0].kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: false
        }
    );
    assert!(!has_more);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn multivalue_counts_include_duplicate_and_null_items_and_distinguish_presence() {
    let mut bytes = complex_links::populated();
    raw::put32(&mut bytes, 10 * PAGE + 16, 4);
    let parents = [
        (1_i32, Some(10_i32)),
        (2, Some(20)),
        (3, Some(30)),
        (4, None),
    ]
    .into_iter()
    .map(|(id, key)| {
        let mut row = 2_u16.to_le_bytes().to_vec();
        row.extend(id.to_le_bytes());
        row.extend(key.unwrap_or(30).to_le_bytes());
        row.push(if key.is_some() { 3 } else { 1 });
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&raw::data_page(10, &parents));
    raw::put32(&mut bytes, 20 * PAGE + 16, 4);
    let children = [
        (10_i32, Some(100_i32), 1_i32),
        (10, Some(100), 2),
        (10, None, 3),
        (20, Some(200), 4),
    ]
    .into_iter()
    .map(|(owner, value, id)| {
        let mut row = 3_u16.to_le_bytes().to_vec();
        row.extend(owner.to_le_bytes());
        row.extend(value.unwrap_or(101).to_le_bytes());
        row.extend(id.to_le_bytes());
        row.push(if value.is_some() { 7 } else { 5 });
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&raw::data_page(20, &children));
    let (_directory, path, worker) = opened(&bytes);
    let Response::Data { rows, has_more, .. } = page(&worker, 0).unwrap() else {
        panic!("expected complete collection summaries");
    };
    for (row, (assigned, members)) in rows
        .iter()
        .zip([(true, 3), (true, 1), (true, 0), (false, 0)])
    {
        assert_eq!(row[1].kind, CellKind::Multivalue { assigned, members });
    }
    assert_eq!(rows[2][1].text, "0 values");
    assert_eq!(rows[3][1].text, "NULL");
    assert!(!has_more);

    // Page-local indices must retain the global parent offset association.
    let Response::Data { rows, offset, .. } = page(&worker, 2).unwrap() else {
        panic!("expected offset collection summaries");
    };
    assert_eq!(offset, 2);
    assert_eq!(rows[0][0].text, "3");
    assert_eq!(
        rows[0][1].kind,
        CellKind::Multivalue {
            assigned: true,
            members: 0
        }
    );
    assert_eq!(rows[1][0].text, "4");
    assert_eq!(
        rows[1][1].kind,
        CellKind::Multivalue {
            assigned: false,
            members: 0
        }
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn attachment_summary_counts_null_file_data_without_decoding_or_opening_it() {
    let bytes = attachment_modern::database(6);
    let (_directory, path, worker) = opened(&bytes);
    let Response::Data { rows, .. } = page(&worker, 0).unwrap() else {
        panic!("expected attachment summaries");
    };
    // Parent1's second child has NULL FileData. It remains a real attachment.
    for (row, members) in rows.iter().zip([2, 1, 0]) {
        assert_eq!(
            row[1].kind,
            CellKind::Attachments {
                assigned: true,
                members
            }
        );
    }
    assert_eq!(rows[0][1].text, "2 attachments");
    assert_eq!(rows[1][1].text, "1 attachment");
    assert_eq!(rows[2][1].text, "0 attachments");
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn stored_history_summary_remains_separate_from_visible_current_memo() {
    let bytes = version_history::database();
    let (_directory, path, worker) = opened(&bytes);
    let Response::Data { rows, columns, .. } = page(&worker, 0).unwrap() else {
        panic!("expected history summaries");
    };
    assert_eq!(columns, ["Id", "Score", "Notes"]);
    for (row, members) in rows.iter().zip([2, 2, 0]) {
        assert_eq!(
            row[1].kind,
            CellKind::VersionHistory {
                assigned: true,
                members
            }
        );
        assert_eq!(
            row[2].kind,
            CellKind::Scalar {
                is_null: true,
                stored_calculation: false
            }
        );
        assert_eq!(row[2].text, "NULL");
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn invalid_child_snapshot_returns_error_instead_of_partial_counts() {
    let mut bytes = attachment_modern::database(6);
    bytes[22 * PAGE] = 0;
    let (_directory, path, worker) = opened(&bytes);
    assert!(matches!(
        page(&worker, 0),
        Err(Error::Native(InspectError::Malformed))
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
