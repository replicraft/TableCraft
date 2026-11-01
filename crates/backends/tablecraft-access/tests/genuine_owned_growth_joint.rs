#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::genuine_owned_growth_joint as source;

const SCOPE: CatalogScope = CatalogScope::new(148);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(bytes: &[u8], expected: &str) {
    let actual = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn rows(id: Option<i32>, marker: &str) -> Vec<Vec<Value>> {
    vec![vec![
        id.map_or(Value::Null, Value::Int32),
        Value::Text(marker.into()),
        Value::Binary(vec![199, 0, 255]),
    ]]
}

#[test]
#[ignore = "requires both genuine originals and all four pinned joint AFTERs"]
fn joint_growth_typed_and_sql_preserve_original_rhs_in_both_orders() {
    let cases = source::load_all(verify);
    let mut saves = 0;
    for case in cases {
        for typed in [true, false] {
            for reverse in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("Sentinel", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(
                        editor.snapshot().rows(),
                        rows(Some(23), "unchanged sentinel")
                    );
                    let handle = editor.snapshot().row_handle(0).unwrap();
                    let marker = [CellUpdate::new(
                        handle,
                        Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
                    )];
                    let id = [CellUpdate::new(
                        handle,
                        case.id.map_or(Value::Null, Value::Int32),
                    )];
                    let mut columns = [
                        ColumnUpdates {
                            column: "Marker",
                            updates: &marker,
                        },
                        ColumnUpdates {
                            column: "ID",
                            updates: &id,
                        },
                    ];
                    if reverse {
                        columns.reverse();
                    }
                    editor
                        .plan_columns(&columns, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let id = case.id.map_or("NULL".into(), |value| value.to_string());
                    let assignments = if reverse {
                        format!("[ID]={id},Marker='aaaaaaaaaaaaaaaaaaa'")
                    } else {
                        format!("Marker='aaaaaaaaaaaaaaaaaaa',[ID]={id}")
                    };
                    let sql = format!("UPDATE Sentinel SET {assignments} WHERE [ID]=23");
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = session
                    .edit_table("Sentinel", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    editor.snapshot().rows(),
                    rows(case.id, "aaaaaaaaaaaaaaaaaaa")
                );
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 16);
}
