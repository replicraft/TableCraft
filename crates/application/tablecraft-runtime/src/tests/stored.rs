use super::{open, request};
use crate::{CellKind, Command, Error, Response, Worker};
use std::path::PathBuf;
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{
    attachment_modern, calculated_storage, complex_links, version_history,
};

const PAGE: usize = 4096;

pub(super) fn opened(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, Worker, u64) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stored.accdb");
    std::fs::write(&path, bytes).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected opened fixture");
    };
    (directory, path, worker, document_revision)
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
    let (_directory, path, worker, _revision) = opened(&bytes);
    let Response::Data { rows, has_more, .. } = page(&worker, 0).unwrap() else {
        panic!("expected stored values");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1].text, "77");
    assert_eq!(
        rows[0][1].kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: true,
            boolean: None,
            display_control: crate::DisplayControl::Absent
        }
    );
    assert_eq!(rows[1][1].text, "NULL");
    assert_eq!(
        rows[1][1].kind,
        CellKind::Scalar {
            is_null: true,
            stored_calculation: true,
            boolean: None,
            display_control: crate::DisplayControl::Absent
        }
    );
    assert_eq!(
        rows[0][0].kind,
        CellKind::Scalar {
            is_null: false,
            stored_calculation: false,
            boolean: None,
            display_control: crate::DisplayControl::Absent
        }
    );
    assert!(!has_more);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn multivalue_counts_include_duplicate_and_null_items_and_distinguish_presence() {
    let bytes = complex_links::presence_cases();
    let (_directory, path, worker, _revision) = opened(&bytes);
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
    let (_directory, path, worker, _revision) = opened(&bytes);
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
    let (_directory, path, worker, _revision) = opened(&bytes);
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
                stored_calculation: false,
                boolean: None,
                display_control: crate::DisplayControl::Absent
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
    let (_directory, path, worker, _revision) = opened(&bytes);
    assert!(matches!(
        page(&worker, 0),
        Err(Error::Native(InspectError::Malformed))
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
