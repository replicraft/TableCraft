#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, SessionState},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, SnapshotError};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_compound_root_split as sources;

const SCOPE: CatalogScope = CatalogScope::new(225);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn payload(seed: usize) -> Vec<u8> {
    (0..84)
        .map(|position| (position * 31 + seed) as u8)
        .collect()
}

fn rows(inserted: bool) -> Vec<Vec<Value>> {
    let mut rows = (-226_i32..=225)
        .filter(|id| *id != 0)
        .enumerate()
        .map(|(ordinal, id)| vec![Value::Int32(id), Value::Binary(payload(ordinal))])
        .collect::<Vec<_>>();
    if inserted {
        rows.push(vec![Value::Int32(226), Value::Binary(payload(226))]);
    }
    rows
}

#[test]
#[ignore = "requires four compound full84 source views qualified before producers"]
fn typed_compound_promotion_refreshes_session_and_empty_delete_preserves_all_records_maps_and_tails()
 {
    let cases = sources::load_all(verify);
    let mut commits = 0;
    for case in cases {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("BinaryStorage", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(snapshot.rows(), rows(false));
        let old = snapshot.row_handle(0).unwrap();
        let staged = editor
            .into_session()
            .insert_table("BinaryStorage", &mut budget(), &|| false)
            .unwrap()
            .plan_insert(
                vec![Value::Int32(226), Value::Binary(payload(226))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(staged.inserted_rows(), 1);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert_eq!(
            fs::metadata(&path).unwrap().len(),
            (case.before.len() + 3 * 4096) as u64
        );
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        commits += 1;

        let editor = session
            .edit_table("BinaryStorage", &mut budget(), &|| false)
            .unwrap();
        let snapshot = EditableTable::snapshot(&editor);
        assert_eq!(snapshot.rows(), rows(true));
        assert_eq!(snapshot.resolve(old), Err(SnapshotError::StaleRow));
        let indexed = tablecraft_ace::read_table_index(
            &mut Cursor::new(fs::read(&path).unwrap()),
            SCOPE,
            "BinaryStorage",
            "PrimaryID",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexed.rows(), rows(true));
        let failure = DeletableTable::plan_deletes(editor, &[old], &mut budget(), &|| false)
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
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

        // This is truly empty: no active key disappears or becomes tail data.
        // It uses the already qualified split AFTER as both BEFORE and AFTER.
        let editor = failure
            .into_session()
            .edit_table("BinaryStorage", &mut budget(), &|| false)
            .unwrap();
        let empty = DeletableTable::plan_deletes(editor, &[], &mut budget(), &|| false).unwrap();
        assert_eq!((empty.matched_rows(), empty.deleted_rows()), (0, 0));
        let (session, noop) = empty.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(noop.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(noop.matched_rows(), Some(0));
        assert_eq!(noop.changed_rows(), Some(0));
        assert_eq!(session.last_completed(), Some(noop));
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        commits += 1;
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert_eq!(commits, 4);
}
