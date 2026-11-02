#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_indexed_insert_association as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(177);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], wanted: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, wanted);
}

#[test]
#[ignore = "requires four source/SDK-qualified cross-page INSERT association views"]
fn retained_typed_and_sql_single_row_consumers_reach_the_same_literal_batch_image() {
    let mut commits = 0;
    for case in fixtures::load_all(verify) {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let mut session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let mut predecessor = case.before.clone();
            for second in [false, true] {
                // Existing host INSERT contracts accept one complete row per
                // command. This is two successive saves on the same lease;
                // the native companion separately exercises one atomic batch.
                let staged = if typed {
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(fixtures::values(second), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let (id, score, label) = if second {
                        (11, 18, "new".to_owned())
                    } else {
                        (10, 17, "L".repeat(200))
                    };
                    let sql = format!(
                        "INSERT INTO Items ([Label], [Guard], [Score], [Id]) VALUES ('{label}', 7, {score}, {id})"
                    );
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), predecessor);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (next, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(next.last_completed(), Some(receipt));
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                predecessor = fs::read(&path).unwrap();
                let editor = next.edit_table("Items", &mut budget(), &|| false).unwrap();
                let rows = EditableTable::snapshot(&editor).rows();
                if second {
                    assert_eq!(predecessor, case.after);
                    assert_eq!(rows, fixtures::rows(true));
                } else {
                    let mut expected = fixtures::rows(false);
                    expected.push(fixtures::values(false));
                    assert_eq!(rows, expected);
                    assert_eq!(rows.len(), 10);
                }
                session = editor.into_session();
                commits += 1;
            }
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
    assert_eq!(commits, 8);
}
