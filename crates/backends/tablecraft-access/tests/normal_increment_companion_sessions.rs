#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Actual retained omission-aware typed/SQL boundaries consume original presence.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase},
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, EditableTable, InsertValue, OmittingInsertableTable, SnapshotError,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_increment_companions as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires companion original20/AFTER20 native and SDK qualification"]
fn generated_companions_save_twenty_typed_and_twelve_sql_images_with_retained_receipts() {
    let cases = fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    });
    let mut saves = [0, 0];
    let mut stale_handles = 0;

    for case in &cases {
        for typed in [true, false] {
            if !typed && case.sql().is_none() {
                continue;
            }

            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, CatalogScope::new(238), &mut budget(), &|| false)
                    .unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.rows(false));
            let stale = if case.old_live == 0 {
                None
            } else {
                Some(editor.snapshot().row_handle(0).unwrap())
            };
            let session = editor.into_session();

            let staged = if typed {
                let values = case
                    .presence()
                    .into_iter()
                    .map(|value| value.map_or(InsertValue::Omitted, InsertValue::Supplied))
                    .collect();
                session
                    .insert_table(case.table, &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert_with_omissions(values, &mut budget(), &|| false)
                    .unwrap()
            } else {
                let query = parse_query(case.sql().unwrap(), ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(staged.inserted_rows(), 1);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.snapshot().rows(), case.rows(true));
            let session = if let Some(handle) = stale {
                let failure = editor
                    .plan_updates(
                        case.columns()[2],
                        &[CellUpdate::new(handle, Value::Null)],
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
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.after);
                assert!(!bundle.exists());
                stale_handles += 1;
                failure.into_session()
            } else {
                editor.into_session()
            };

            let mut invalid = case.presence();
            invalid[case.generated_position] = Some(Value::Null);
            let failure = session
                .insert_table(case.table, &mut budget(), &|| false)
                .unwrap()
                .plan_insert_with_omissions(
                    invalid
                        .into_iter()
                        .map(|value| value.map_or(InsertValue::Omitted, InsertValue::Supplied))
                        .collect(),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                failure.cause(),
                Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                    EditError::Unsupported(WriteBlocker::Column)
                )))
            ));
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }

    assert_eq!(saves, [20, 12]);
    assert_eq!(stale_handles, 24);
}
