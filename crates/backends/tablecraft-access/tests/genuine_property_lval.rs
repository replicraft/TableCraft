#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_property_lval::{self as sources, Recipe};

const SCOPE: CatalogScope = CatalogScope::new(135);

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

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn sql(recipe: Recipe) -> &'static str {
    match recipe {
        Recipe::Same => "UPDATE Sentinel SET Marker='qualified sentinel' WHERE [ID]=23",
        Recipe::Shorter => "UPDATE Sentinel SET Marker='changed sentinel' WHERE [ID]=23",
        Recipe::Empty => "UPDATE Sentinel SET Marker='' WHERE [ID]=23",
        Recipe::Clear => "UPDATE Sentinel SET Marker=NULL WHERE [ID]=23",
        Recipe::Noop => "UPDATE Sentinel SET Marker=Marker WHERE [ID]=23",
        Recipe::EmptyGroup => "UPDATE Sentinel SET Marker=Marker WHERE [ID]=-1",
        _ => panic!("SQL has no invented stale-expected parameter"),
    }
}

fn rows(marker: Value) -> Vec<Vec<Value>> {
    vec![vec![
        Value::Int32(23),
        marker,
        Value::Binary(vec![199, 0, 255]),
    ]]
}

#[test]
#[ignore = "requires the frozen original and twelve literal expectation files"]
fn genuine_property_lval_sessions_save_twelve_typed_and_twelve_sql_images() {
    // Every source and outcome is SHA-checked before opening the first target.
    let cases = sources::load_all(verify);
    let mut saves = [0; 2];
    for case in cases.iter().filter(|case| case.after.is_some()) {
        let after = case.after.as_ref().unwrap();
        for typed in [true, false] {
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
                assert_eq!(editor.snapshot().rows(), rows(case.expected.clone()));
                let updates = if case.empty {
                    vec![]
                } else {
                    vec![CellUpdate::new(
                        editor.snapshot().row_handle(0).unwrap(),
                        case.replacement.clone(),
                    )]
                };
                editor
                    .plan_columns(
                        &[ColumnUpdates {
                            column: "Marker",
                            updates: &updates,
                        }],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(sql(case.recipe), ParseLimits::default()).unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!(staged.matched_rows(), usize::from(!case.empty));
            assert_eq!(staged.changed_rows(), case.changed_rows);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            locked(&path);
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap().as_slice(), after.as_slice());
            assert_eq!(
                &after[case.property_offset..case.property_offset + 57],
                &case.before[case.property_offset..case.property_offset + 57]
            );
            assert!(!bundle.exists());
            locked(&path);
            let editor = session
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            let marker = if case.empty {
                case.expected.clone()
            } else {
                case.replacement.clone()
            };
            assert_eq!(editor.snapshot().rows(), rows(marker));
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [12, 12]);
}

#[test]
#[ignore = "requires the frozen original and twelve literal expectation files"]
fn genuine_property_lval_declared_width_and_stale_handles_keep_files_leases_and_receipts() {
    let cases = sources::load_all(verify);
    for case in cases.iter().filter(|case| case.recipe == Recipe::Overflow) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let updates = [CellUpdate::new(
            editor.snapshot().row_handle(0).unwrap(),
            Value::Text("a".repeat(41)),
        )];
        let failure = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Marker",
                    updates: &updates,
                }],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("forty-one UTF16 units exceed the declared forty-unit field width");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                EditError::Unsupported(WriteBlocker::Column)
            )))
        ));
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        locked(&path);
        failure.into_session().release().unwrap();
    }
    for case in cases.iter().filter(|case| case.recipe == Recipe::Same) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let updates = [CellUpdate::new(stale, case.replacement.clone())];
        let staged = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Marker",
                    updates: &updates,
                }],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Sentinel", &mut budget(), &|| false)
            .unwrap();
        let updates = [CellUpdate::new(stale, Value::Text("x".into()))];
        let failure = editor
            .plan_columns(
                &[ColumnUpdates {
                    column: "Marker",
                    updates: &updates,
                }],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("a prior snapshot cannot supply a current handle");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(
            fs::read(&path).unwrap().as_slice(),
            case.after.as_ref().unwrap().as_slice()
        );
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}
