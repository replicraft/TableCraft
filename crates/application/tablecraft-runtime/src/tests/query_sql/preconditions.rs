//! Invalid source ownership refuses before untrusted editor text is parsed.
use super::{Command, Error, Response, SavedQueryTarget, Worker, opened, request, storage_pair};
use crate::tests::open;
use std::{fs, io::Cursor};
use tablecraft_ace::{EncryptionRewrite, rewrite_encryption};

#[test]
fn stale_document_precedes_original_and_proposed_sql_errors() {
    let [before, _] = storage_pair();
    let (directory, path, worker, mut target) = opened(&before);
    target.document_revision += 1;
    let oversized = "x".repeat(65_537);
    for invalid in ["SELECT (", oversized.as_str()] {
        for (original, proposed) in [
            (invalid, "SELECT 43 AS Answer"),
            ("SELECT 42 AS Answer", invalid),
        ] {
            assert!(matches!(
                request(
                    &worker,
                    Command::ReviewQuerySql {
                        // NO_CLONE: Each consumed worker command owns its target;
                        // the test retains the same stale identity for both inputs.
                        target: target.clone(),
                        original_sql: original.into(),
                        proposed_sql: proposed.into(),
                    }
                ),
                Err(Error::StaleSavedQuery)
            ));
        }
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn authenticated_read_only_source_precedes_invalid_sql_without_issuing_a_review() {
    let [before, _] = storage_pair();
    // This derivative supplies an authenticated source for runtime admission;
    // independent engine fixtures establish encryption correctness separately.
    let encrypted = rewrite_encryption(
        &mut Cursor::new(before),
        EncryptionRewrite::Add {
            password: None,
            new_password: "test-only credential",
        },
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("read-only.accdb");
    fs::write(&path, encrypted.as_bytes()).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(&worker, open(&path, "test-only credential")).unwrap()
    else {
        panic!("expected authenticated native metadata");
    };
    let query = queries
        .into_iter()
        .find(|query| query.name == "Saved")
        .unwrap();
    let target = SavedQueryTarget {
        document_revision,
        object_id: query.object_id,
        name: query.name,
    };
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQuerySql {
                target,
                original_sql: "SELECT (".into(),
                proposed_sql: "SELECT (".into(),
            }
        ),
        Err(Error::Edit(crate::EditError::ReadOnlySource))
    ));
    assert_eq!(fs::read(&path).unwrap(), encrypted.as_bytes());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}
