#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, EditableTable, SnapshotError};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_node_retirement as sources;

const SCOPE: CatalogScope = CatalogScope::new(164);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let digest = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(digest, expected);
}

#[test]
#[ignore = "requires eight independent native/SDK-qualified NODE retirement source images"]
fn retained_node_typed_sql_saves_preserve_both_fences_and_snapshot_handle_boundaries() {
    let cases = sources::load_all(verify);
    let mut commits = 0;
    let mut empty_commits = 0;
    for case in cases {
        let selected = case.recipe.selected().collect::<Vec<_>>();
        for typed in [true, false] {
            let orders: &[bool] = if typed { &[false, true] } else { &[false] };
            for &reverse in orders {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                let snapshot = EditableTable::snapshot(&editor);
                assert_eq!(snapshot.rows(), sources::rows(None, false));
                let old = snapshot.row_handle(selected[0]).unwrap();
                let mut handles = selected
                    .iter()
                    .map(|slot| snapshot.row_handle(*slot).unwrap())
                    .collect::<Vec<_>>();
                if reverse {
                    handles.reverse();
                }
                let staged = if typed {
                    DeletableTable::plan_deletes(editor, &handles, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(case.recipe.sql(), ParseLimits::default()).unwrap();
                    editor
                        .into_session()
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                let removed = selected.len();
                assert_eq!(
                    (staged.matched_rows(), staged.deleted_rows()),
                    (removed, removed)
                );
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), Some(removed));
                assert_eq!(receipt.changed_rows(), Some(removed));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                commits += 1;

                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    EditableTable::snapshot(&editor).rows(),
                    sources::rows(Some(case.recipe), false)
                );
                assert_eq!(
                    EditableTable::snapshot(&editor).resolve(old),
                    Err(SnapshotError::StaleRow)
                );
                let failure =
                    DeletableTable::plan_deletes(editor, &[old], &mut budget(), &|| false)
                        .err()
                        .unwrap();
                assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                        SnapshotError::StaleRow
                    )))
                ));
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

                // Fresh empty work must retain the exact qualified retired topology,
                // including the all-deleted table and both logical aliases.
                let session = failure.into_session();
                let empty = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    DeletableTable::plan_deletes(editor, &[], &mut budget(), &|| false).unwrap()
                } else {
                    let query =
                        parse_query("DELETE FROM Items WHERE [Id]=-1", ParseLimits::default())
                            .unwrap();
                    session
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((empty.matched_rows(), empty.deleted_rows()), (0, 0));
                let (session, noop) = empty.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(noop.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(noop.matched_rows(), Some(0));
                assert_eq!(noop.changed_rows(), Some(0));
                assert_eq!(session.last_completed(), Some(noop));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                empty_commits += 1;
            }
        }
    }
    assert_eq!((commits, empty_commits), (18, 18));
}
