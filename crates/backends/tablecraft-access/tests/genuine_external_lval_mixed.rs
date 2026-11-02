#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_external_lval_mixed as fixture;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    assert_eq!(
        Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        expected
    );
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn retained_external_mixed_typed_and_sql_saves_preserve_full_images_receipts_empty_and_stale_handles()
 {
    let cases = fixture::load_all(verify);
    let mut saves = [0; 2];
    let mut stale_refusals = 0;
    for case in cases.iter().filter(|case| case.recipe != "original") {
        for reverse in [false, true] {
            for typed in [true, false] {
                if !typed && fixture::statement(case, reverse).is_none() {
                    continue;
                }
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                let stale = editor.snapshot().row_handle(2).unwrap();
                let staged = if typed {
                    let requests = fixture::request_groups(case, reverse);
                    let updates = requests
                        .iter()
                        .map(|group| {
                            group
                                .updates
                                .iter()
                                .map(|cell| {
                                    CellUpdate::new(
                                        editor.snapshot().row_handle(cell.row).unwrap(),
                                        cell.replacement.clone(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let columns = requests
                        .iter()
                        .zip(&updates)
                        .map(|(group, updates)| ColumnUpdates {
                            column: group.column,
                            updates,
                        })
                        .collect::<Vec<_>>();
                    editor
                        .plan_columns(&columns, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let session = editor.into_session();
                    let query = parse_query(
                        &fixture::statement(case, reverse).unwrap(),
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session
                        .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                let changed = fixture::changed_rows(case);
                assert_eq!(
                    (staged.matched_rows(), staged.changed_rows()),
                    (changed, changed)
                );
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(
                    (receipt.matched_rows(), receipt.changed_rows()),
                    (Some(changed), Some(changed))
                );
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), case.rows.as_slice());
                let empty = editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Memo",
                            updates: &[],
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!((empty.matched_rows(), empty.changed_rows()), (0, 0));
                let (session, empty_receipt) =
                    empty.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(empty_receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(
                    (empty_receipt.matched_rows(), empty_receipt.changed_rows()),
                    (Some(0), Some(0))
                );
                assert_eq!(session.last_completed(), Some(empty_receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                let editor = session
                    .edit_table("Items", &mut budget(), &|| false)
                    .unwrap();
                let failure = editor
                    .plan_updates(
                        "Memo",
                        &[CellUpdate::new(stale, case.rows[2][3].clone())],
                        &mut budget(),
                        &|| false,
                    )
                    .err()
                    .unwrap();
                assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                        SnapshotError::StaleRow
                    )))
                ));
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), Some(empty_receipt));
                assert_eq!(failure.state(), SessionState::Ready);
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                locked(&path);
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                saves[usize::from(!typed)] += 1;
                stale_refusals += 1;
            }
        }
    }
    assert_eq!(saves, [32, 16]);
    assert_eq!(stale_refusals, 48);
}
