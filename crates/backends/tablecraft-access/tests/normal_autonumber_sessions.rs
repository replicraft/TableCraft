#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Retained typed and SQL omissions use the actual shared TCNA2 consumer.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    journal,
    offline::{Cause, OfflineWriteSession, Phase},
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_normal_autonumber as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

#[test]
#[ignore = "requires14 source native/SDK views before matching normal generation"]
fn retained_typed_and_sql_omissions_save_eight_images_and_preserve_prior_receipt_on_refusal() {
    let sources = sources();
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            for typed in [false, true] {
                let before = sources.bytes(header, recipe.before());
                let expected = sources.bytes(header, recipe.after());
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, before).unwrap();
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(234),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let staged = if typed {
                    session
                        .insert_table("Items", &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert_with_omissions(
                            vec![
                                InsertValue::Omitted,
                                InsertValue::Supplied(Value::Int32(17)),
                            ],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let query = parse_query(
                        "INSERT INTO Items ([Score]) VALUES (17)",
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(EditableTable::snapshot(&editor).rows(), recipe.rows(true));
                let session = editor.into_session();
                let failure = session
                    .insert_table("Items", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert_with_omissions(
                        vec![
                            InsertValue::Supplied(Value::Null),
                            InsertValue::Supplied(Value::Int32(17)),
                        ],
                        &mut budget(),
                        &|| false,
                    )
                    .err()
                    .unwrap();
                assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                        EditError::Unsupported(WriteBlocker::Column)
                    )))
                ));
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 8);
}
