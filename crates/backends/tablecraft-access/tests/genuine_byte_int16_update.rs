#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_byte_int16_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires admitted exact originals and literal endpoint/NULL span states"]
fn retained_typed_small_integral_batches_save_literal_spans_and_refresh_exact_rows() {
    let cases = source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    });
    let mut saves = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let journal = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session =
            OfflineWriteSession::open(&path, CatalogScope::new(296), &mut budget(), &|| false)
                .unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
        assert_eq!(editor.snapshot().rows(), case.rows(false));
        let requests = case.requests();
        let cells: Vec<_> = requests
            .iter()
            .enumerate()
            .map(|(position, (_, before, after))| {
                assert_eq!(editor.snapshot().rows()[2][position + 1], *before);
                vec![CellUpdate::new(
                    editor.snapshot().row_handle(2).unwrap(),
                    after.clone(),
                )]
            })
            .collect();
        let columns: Vec<_> = requests
            .iter()
            .zip(&cells)
            .map(|((column, _, _), updates)| ColumnUpdates { column, updates })
            .collect();
        let staged = editor
            .plan_columns(&columns, &mut budget(), &|| false)
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!journal.exists());
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
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 6);
}
