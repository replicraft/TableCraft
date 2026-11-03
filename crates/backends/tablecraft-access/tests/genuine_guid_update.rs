#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_guid_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires exact qualified GUID originals; canonical typed values save native spans"]
fn typed_guid_saves_preserve_native_permutation_null_payload_and_receipt() {
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
            OfflineWriteSession::open(&path, CatalogScope::new(298), &mut budget(), &|| false)
                .unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
        assert_eq!(editor.snapshot().rows(), case.rows(false));
        let (row, expected, replacement) = case.request();
        assert_eq!(editor.snapshot().rows()[row][9], expected);
        let handle = editor.snapshot().row_handle(row).unwrap();
        let staged = editor
            .plan_updates(
                "GuidValue",
                &[CellUpdate::new(handle, replacement)],
                &mut budget(),
                &|| false,
            )
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
        editor.into_session().release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 6);
}
