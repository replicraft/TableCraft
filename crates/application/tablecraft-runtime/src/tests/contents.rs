use super::{open, query, request, stored::opened};
use crate::{CellKind, CollectionKind, Command, ContentsTarget, Error, Response};
use tablecraft_ace::InspectError;
use tablecraft_testkit::fixtures::access::{
    attachment_modern, attachments, complex_links, mutation_fixture, version_history,
};

fn target(revision: u64, parent_ordinal: u32) -> ContentsTarget {
    ContentsTarget {
        document_revision: revision,
        table: "Items".to_owned(),
        column: "Score".to_owned(),
        parent_ordinal,
    }
}

fn is_null(kind: CellKind) -> bool {
    matches!(
        kind,
        CellKind::Scalar {
            is_null: true,
            stored_calculation: false
        }
    )
}

#[test]
fn multivalue_contents_preserve_duplicates_null_and_global_parent_presence() {
    let bytes = complex_links::presence_cases();
    let (_directory, path, worker, revision) = opened(&bytes);
    let Response::Data {
        source_revision, ..
    } = request(
        &worker,
        Command::Page {
            table: "Items".to_owned(),
            offset: 2,
        },
    )
    .unwrap()
    else {
        panic!("expected native page revision");
    };
    assert_eq!(source_revision, Some(revision));
    let Response::Data {
        source_revision, ..
    } = request(&worker, query("SELECT 1")).unwrap()
    else {
        panic!("expected query result");
    };
    assert_eq!(source_revision, None);

    let original_target = target(revision, 0);
    let Response::Contents {
        target: actual,
        kind,
        assigned,
        columns,
        rows,
    } = request(
        &worker,
        Command::InspectCollection {
            target: original_target.clone(),
        },
    )
    .unwrap()
    else {
        panic!("expected complete multivalue contents");
    };
    assert!(actual == original_target);
    assert_eq!(kind, CollectionKind::Multivalue);
    assert!(assigned);
    assert_eq!(columns, ["Value"]);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0].text, "100");
    assert_eq!(rows[1][0].text, "100");
    assert!(is_null(rows[2][0].kind));

    for (ordinal, expected_assigned) in [(2, true), (3, false)] {
        let selected = target(revision, ordinal);
        let Response::Contents {
            target: actual,
            assigned,
            rows,
            ..
        } = request(
            &worker,
            Command::InspectCollection {
                target: selected.clone(),
            },
        )
        .unwrap()
        else {
            panic!("expected empty membership with explicit presence");
        };
        assert!(actual == selected);
        assert_eq!(assigned, expected_assigned);
        assert!(rows.is_empty());
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn attachment_contents_keep_nullable_metadata_and_only_encoded_lengths() {
    let bytes = attachment_modern::database(6);
    let (_directory, path, worker, revision) = opened(&bytes);
    let Response::Contents {
        kind,
        assigned,
        columns,
        rows,
        ..
    } = request(
        &worker,
        Command::InspectCollection {
            target: target(revision, 0),
        },
    )
    .unwrap()
    else {
        panic!("expected stored attachment metadata");
    };
    assert_eq!(kind, CollectionKind::Attachments);
    assert!(assigned);
    assert_eq!(columns.len(), 8);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "synthetic.bin");
    assert_eq!(rows[0][3].text, "relative?x");
    assert_eq!(rows[0][5].text, format!("0x{:016x}", (-0.25_f64).to_bits()));
    assert_eq!(rows[0][7].text, attachments::PAYLOAD.len().to_string());
    assert_eq!(rows[1][0].text, "2");
    // This row retains nonzero inactive date/flag bytes. Only actual nullable
    // accessors determine the result, never its raw fixed-region bytes.
    for cell in rows[1].iter().skip(1) {
        assert!(is_null(cell.kind));
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn history_contents_keep_stored_order_null_empty_and_exact_date_bits() {
    let bytes = version_history::database_with(&[
        (
            10,
            9,
            Some("later stored first"),
            Some((-0.0_f64).to_bits()),
        ),
        (10, 2, Some(""), Some(1)),
        (10, 7, None, None),
    ]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let Response::Contents {
        kind,
        assigned,
        rows,
        ..
    } = request(
        &worker,
        Command::InspectCollection {
            target: target(revision, 0),
        },
    )
    .unwrap()
    else {
        panic!("expected stored history");
    };
    assert_eq!(kind, CollectionKind::VersionHistory);
    assert!(assigned);
    assert_eq!(
        rows.iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["9", "2", "7"]
    );
    assert_eq!(rows[0][3].text, "0x8000000000000000");
    assert_eq!(rows[1][3].text, "0x0000000000000001");
    assert_eq!(rows[1][1].text, "");
    assert!(!is_null(rows[1][1].kind));
    assert!(is_null(rows[2][1].kind));
    assert!(is_null(rows[2][2].kind));
    assert!(is_null(rows[2][3].kind));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn original_column_parent_and_collection_roles_are_checked() {
    let bytes = complex_links::populated();
    let (_directory, path, worker, revision) = opened(&bytes);
    let mut missing = target(revision, 0);
    missing.column = "score".to_owned();
    assert!(matches!(
        request(&worker, Command::InspectCollection { target: missing }),
        Err(Error::CollectionColumnMissing)
    ));
    let mut scalar = target(revision, 0);
    scalar.column = "Id".to_owned();
    assert!(matches!(
        request(&worker, Command::InspectCollection { target: scalar }),
        Err(Error::NotCollection)
    ));
    assert!(matches!(
        request(
            &worker,
            Command::InspectCollection {
                target: target(revision, u32::MAX)
            }
        ),
        Err(Error::CollectionParentMissing)
    ));
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let Response::Opened {
        document_revision: plain_revision,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected scalar document");
    };
    let mut scalar = target(plain_revision, 0);
    scalar.column = "Score".to_owned();
    assert!(matches!(
        request(&worker, Command::InspectCollection { target: scalar }),
        Err(Error::NotCollection)
    ));
}

#[test]
fn reopen_attempts_invalidate_old_targets_before_collection_reads() {
    let bytes = complex_links::populated();
    let (directory, path, worker, first) = opened(&bytes);
    let old = target(first, 0);
    let Response::Opened {
        document_revision: second,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected reopened snapshot");
    };
    assert_eq!(second, first + 1);
    assert!(matches!(
        request(&worker, Command::InspectCollection { target: old }),
        Err(Error::StaleDocument)
    ));
    let selected = target(second, 0);
    assert!(matches!(
        request(
            &worker,
            Command::InspectCollection {
                target: selected.clone()
            }
        ),
        Ok(Response::Contents { .. })
    ));
    assert!(matches!(
        request(&worker, open(&directory.path().join("missing.accdb"), "")),
        Err(Error::Io(_))
    ));
    assert!(matches!(
        request(&worker, Command::InspectCollection { target: selected }),
        Err(Error::StaleDocument)
    ));
    let Response::Opened {
        document_revision: fourth,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected successful retry");
    };
    assert_eq!(fourth, second + 2);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn malformed_child_membership_never_becomes_partial_or_empty_contents() {
    let mut bytes = attachment_modern::database(6);
    bytes[22 * 4096] = 0;
    let (_directory, path, worker, revision) = opened(&bytes);
    assert!(matches!(
        request(
            &worker,
            Command::InspectCollection {
                target: target(revision, 0)
            }
        ),
        Err(Error::Native(InspectError::Malformed))
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
