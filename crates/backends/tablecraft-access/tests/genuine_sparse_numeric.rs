#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::qualified_sparse_numeric as sources;

const SCOPE: CatalogScope = CatalogScope::new(201);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| assert_eq!(digest(raw), expected))
}

fn unchanged(path: &Path, bundle: &Path, before: &[u8]) {
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!bundle.exists());
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
#[ignore = "requires source-qualified sparse action literals and new action-frame proof"]
fn sparse_typed_sql_actions_preserve_whole_images_without_granting_update_capability() {
    let mut saves = [0; 2];
    for case in cases() {
        for typed in [true, false] {
            let insert = case.recipe.starts_with("insert-");
            if !typed && insert && case.recipe != "insert-null" {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let bundle = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let changed = if insert { 1 } else { 3 };
            let (session, receipt) = if insert {
                let staged = if typed {
                    session
                        .insert_table(case.table, &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(case.request(), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let sql = format!(
                        "INSERT INTO {} ([Marker],[KeyValue],[Id]) VALUES (68,NULL,4)",
                        case.table
                    );
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            } else {
                let staged = if typed {
                    let editor = session
                        .edit_table(case.table, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::Unavailable);
                    let snapshot = EditableTable::snapshot(&editor);
                    assert_eq!(snapshot.rows().len(), 3);
                    let handles = (0..3)
                        .map(|ordinal| snapshot.row_handle(ordinal).unwrap())
                        .collect::<Vec<_>>();
                    DeletableTable::plan_deletes(editor, &handles, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(
                        &format!("DELETE FROM {}", case.table),
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.deleted_rows()), (3, 3));
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(changed));
            assert_eq!(receipt.matched_rows(), if insert { None } else { Some(3) });
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(EditableTable::snapshot(&editor).rows(), case.rows());
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [36, 24]);
}
