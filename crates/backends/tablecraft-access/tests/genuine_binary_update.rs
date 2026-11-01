#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_binary_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE source views"]
fn binary_typed_and_sql_null_updates_publish_exact_images_and_refresh_the_held_session() {
    let mut saves = [0; 2];
    for case in source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        )
    }) {
        for typed in [true, false] {
            if !typed && source::sql(case.recipe).is_none() {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let journal = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(218), &mut budget(), &|| false)
                    .unwrap();
            let staged = if typed {
                let editor = session
                    .edit_table("BinaryKeys", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                let cells = source::updates(case.recipe)
                    .into_iter()
                    .map(|(ordinal, expected, value)| {
                        assert_eq!(editor.snapshot().rows()[ordinal][1], expected);
                        CellUpdate::new(editor.snapshot().row_handle(ordinal).unwrap(), value)
                    })
                    .collect::<Vec<_>>();
                editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "KeyValue",
                            updates: &cells,
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query =
                    parse_query(source::sql(case.recipe).unwrap(), ParseLimits::default()).unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!(
                (staged.matched_rows(), staged.changed_rows()),
                (case.changed_rows(), case.changed_rows())
            );
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!journal.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), Some(case.changed_rows()));
            assert_eq!(receipt.changed_rows(), Some(case.changed_rows()));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!journal.exists());
            let editor = session
                .edit_table("BinaryKeys", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), source::final_rows(case.recipe));
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [14, 4]);
}
