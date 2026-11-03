#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ieee_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires six qualified existing IEEE source images"]
fn typed_ieee_batches_save_exact_bits_and_preserve_session_receipt_and_stale_handles() {
    let cases = source::load_all(|raw, expected| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    });
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(292), &mut budget(), &|| false)
                .unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
        assert_eq!(editor.snapshot().rows(), case.rows(false));
        let stale = editor.snapshot().row_handle(2).unwrap();
        let groups = case.groups();
        let cells: Vec<_> = groups
            .iter()
            .map(|group| {
                let position = match group.column {
                    "SingleValue" => 6,
                    "DoubleValue" => 7,
                    "DateValue" => 8,
                    _ => unreachable!(),
                };
                assert_eq!(editor.snapshot().rows()[2][position], group.expected);
                vec![CellUpdate::new(stale, group.replacement.clone())]
            })
            .collect();
        let columns: Vec<_> = groups
            .iter()
            .zip(&cells)
            .map(|(group, updates)| ColumnUpdates {
                column: group.column,
                updates,
            })
            .collect();
        let staged = editor
            .plan_columns(&columns, &mut budget(), &|| false)
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!journal.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), Some(1));
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.snapshot().rows(), case.rows(true));
        let stale = [CellUpdate::new(stale, groups[0].replacement.clone())];
        let failure = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "SingleValue",
                    updates: &stale,
                }],
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
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!journal.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}
