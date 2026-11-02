//! Shared Access-session plumbing for two independently pinned placement families.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

pub fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn row(marker: &str) -> Vec<Vec<Value>> {
    vec![vec![
        Value::Int32(23),
        Value::Text(marker.into()),
        Value::Binary(vec![199, 0, 255]),
    ]]
}

pub fn run<'a>(
    cases: impl IntoIterator<Item = (&'a [u8], &'a [u8], &'a [u8])>,
    markers: [&str; 3],
    scope: CatalogScope,
) -> usize {
    let mut saves = 0;
    for (before, grown, second) in cases {
        for typed in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&path, before).unwrap();
            let mut session =
                OfflineWriteSession::open(&path, scope, &mut budget(), &|| false).unwrap();
            for (old, new, expected_before, expected_after) in [
                (markers[0], markers[1], before, grown),
                (markers[1], markers[2], grown, second),
            ] {
                let staged = if typed {
                    let editor = session
                        .edit_table("Sentinel", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), row(old));
                    let handle = editor.snapshot().row_handle(0).unwrap();
                    editor
                        .plan_updates(
                            "Marker",
                            &[CellUpdate::new(handle, Value::Text(new.into()))],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let sql = format!("UPDATE Sentinel SET Marker='{new}' WHERE [ID]=23");
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
                assert_eq!(fs::read(&path).unwrap().as_slice(), expected_before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (next, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(next.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap().as_slice(), expected_after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = next
                    .edit_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.snapshot().rows(), row(new));
                session = editor.into_session();
                saves += 1;
            }
            session.release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }
    saves
}
