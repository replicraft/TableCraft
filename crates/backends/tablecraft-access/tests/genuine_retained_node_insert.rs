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
use tablecraft_testkit::fixtures::access::genuine_retained_node_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(191);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let hash = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(hash, expected);
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn typed_and_sql_node_insert_keep_one_lease_and_literal_whole_images() {
    let mut commits = 0;
    for case in sources::load_all(verify) {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let inserter = session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    inserter
                        .columns()
                        .iter()
                        .map(|field| field.name())
                        .collect::<Vec<_>>(),
                    ["Id", "Score", "Guard"]
                );
                inserter
                    .plan_insert(sources::values(), &mut budget(), &|| false)
                    .unwrap()
            } else {
                let query = parse_query(
                    "INSERT INTO Items ([Guard],[Score],[Id]) VALUES (1,400,801)",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                EditableTable::snapshot(&editor).rows(),
                sources::rows(true, false)
            );
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            commits += 1;
        }
    }
    assert_eq!(commits, 4);
}
