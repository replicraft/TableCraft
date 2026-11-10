//! The native read envelope is independent from standard mutation-image admission.
use super::{open, request};
use crate::{Command, QueryRunOptions, Response, Worker};
use std::{fs::File, io::Write};
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn a_sparse_database_above_the_former_capture_quota_opens_and_reads_without_writer_expansion() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("larger-read.mdb");
    let original = mutation_fixture::database();
    let mut file = File::create(&path).unwrap();
    file.write_all(&original).unwrap();
    file.set_len(65 * 1024 * 1024).unwrap();
    drop(file);
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision,
        table_creation_available,
        table_design_available,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected native source opening");
    };
    assert!(!table_creation_available);
    assert!(!table_design_available);
    let Response::Data { rows, has_more, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected stored native records");
    };
    assert_eq!(rows.len(), 3);
    assert!(!has_more);
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Query {
            sql: "SELECT COUNT(*) FROM Items".into(),
            options: QueryRunOptions::default(),
        },
    )
    .unwrap() else {
        panic!("expected a complete count query");
    };
    assert_eq!(rows[0][0].text, "3");
    #[cfg(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    ))]
    assert!(matches!(
        request(
            &worker,
            Command::BeginEdit {
                target: crate::EditTarget {
                    document_revision,
                    table: "Items".into(),
                    column: "Score".into(),
                    parent_ordinal: 0
                }
            }
        ),
        Err(crate::Error::Edit(crate::EditError::ReadOnlySource))
    ));
    #[cfg(not(all(
        feature = "offline-edit",
        any(target_os = "macos", target_os = "linux", windows)
    )))]
    let _ = document_revision;
    let after = std::fs::read(&path).unwrap();
    assert_eq!(after.len(), 65 * 1024 * 1024);
    assert_eq!(&after[..original.len()], original);
    assert!(after[original.len()..].iter().all(|byte| *byte == 0));
}
