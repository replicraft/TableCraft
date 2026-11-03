#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_indexed_guid_update::requests::{
    self as source, Case,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn save(case: &Case, score_first: Option<bool>) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target.accdb");
    let journal = directory.path().join("journal");
    fs::write(&path, &case.before).unwrap();
    let session =
        OfflineWriteSession::open(&path, CatalogScope::new(298), &mut budget(), &|| false).unwrap();
    let editor = session
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
    assert_eq!(editor.snapshot().rows(), case.rows(false));
    let mut updates = Vec::new();
    for (row, expected, replacement) in case.updates() {
        assert_eq!(editor.snapshot().rows()[row][1], expected);
        let handle = editor.snapshot().row_handle(row).unwrap();
        updates.push(CellUpdate::new(handle, replacement));
    }
    let count = case.changed_rows();
    let expected = if score_first.is_some() {
        case.composite_after()
    } else {
        case.after.to_vec()
    };
    let staged = if let Some(reverse) = score_first {
        let score = [CellUpdate::new(
            editor.snapshot().row_handle(0).unwrap(),
            Value::Int32(-63),
        )];
        let guid = ColumnUpdates {
            column: "Group",
            updates: &updates,
        };
        let score = ColumnUpdates {
            column: "Score",
            updates: &score,
        };
        let columns = if reverse {
            [score, guid]
        } else {
            [guid, score]
        };
        editor.plan_columns(&columns, &mut budget(), &|| false)
    } else {
        editor.plan_updates("Group", &updates, &mut budget(), &|| false)
    }
    .unwrap();
    assert_eq!(
        (staged.matched_rows(), staged.changed_rows()),
        (count, count)
    );
    assert_eq!(fs::read(&path).unwrap(), &*case.before);
    assert!(!journal.exists());
    assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

    let (session, receipt) = staged.apply(&journal, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(receipt.matched_rows(), Some(count));
    assert_eq!(receipt.changed_rows(), Some(count));
    assert_eq!(session.state(), SessionState::Ready);
    assert_eq!(session.last_completed(), Some(receipt));
    assert_eq!(fs::read(&path).unwrap(), expected);
    assert!(!journal.exists());
    let editor = session
        .edit_table("Items", &mut budget(), &|| false)
        .unwrap();
    let mut rows = case.rows(true);
    if score_first.is_some() {
        rows[0][2] = Value::Int32(-63);
    }
    assert_eq!(editor.snapshot().rows(), rows);
    editor.into_session().release().unwrap();
    ExclusiveFile::open(&path).unwrap().close().unwrap();
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
#[ignore = "four admitted originals and 28 ordered source-backed GUID replacements"]
fn typed_indexed_guid_batches_save_exact_images_and_refresh_rows() {
    let cases = source::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut counts = [0; 2];
    for case in cases {
        save(&case, None);
        counts[0] += 1;
        if case.kind == 1 {
            for reverse in [false, true] {
                save(&case, Some(reverse));
                counts[1] += 1;
            }
        }
    }
    assert_eq!(counts, [28, 8]);
}
