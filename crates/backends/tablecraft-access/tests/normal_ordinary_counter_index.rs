#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState},
    sql,
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_query::UpdateError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_ordinary_indexed::{self as sources, Case, Source};

const SCOPE: CatalogScope = CatalogScope::new(121);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn source(header: u8, identity: Case) -> Source {
    let mut source = sources::source(identity);
    source.bytes[20] = header;
    source
}

fn locked(path: &std::path::Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn literal_after(source: &Source, replacement: &Value) -> Vec<u8> {
    let mut expected = source.bytes.clone();
    let base = 8 * 4096;
    let start = usize::from(u16::from_le_bytes(
        expected[base + 14..base + 16].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    match (&source.rows[0][source.target_position], replacement) {
        (Value::Int32(before), Value::Int32(after)) => {
            assert_eq!(source.target, "Score");
            assert_eq!(
                &expected[base + start + 6..base + start + 10],
                &before.to_le_bytes()
            );
            expected[base + start + 6..base + start + 10].copy_from_slice(&after.to_le_bytes());
        }
        (Value::Boolean(true), Value::Boolean(false)) => {
            assert_eq!(source.target, "Flag");
            assert_eq!(&expected[base + start..base + start + 2], &[5, 0]);
            assert_eq!(expected[9 * 4096 - 1] & 0x10, 0x10);
            expected[9 * 4096 - 1] &= !0x10;
        }
        _ => panic!("independent source target must be Score or originally true Flag"),
    }
    expected
}

fn value(source: &Source) -> Value {
    if source.target == "Flag" {
        Value::Boolean(false)
    } else {
        Value::Int32(17)
    }
}

fn adapter_error(failure: &Failure) -> &journal::Error {
    match failure.cause() {
        Cause::Editing(error) => error,
        Cause::Planning(sql::RecoverableError::Plan(UpdateError::Backend(error))) => error,
        cause => panic!("expected a typed retained native refusal: {cause:?}"),
    }
}

#[test]
fn retained_counter_index_typed_and_sql_sessions_save_thirty_six_complete_images() {
    let mut saves = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            let source = source(header, identity);
            if !source.ordinary {
                continue;
            }
            let replacement = value(&source);
            let expected = literal_after(&source, &replacement);
            let mut rows = source.rows.clone();
            rows[0][source.target_position] = replacement.clone();
            for typed in [true, false] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table(source.table, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), source.rows);
                    let handle = editor.snapshot().row_handle(0).unwrap();
                    editor
                        .plan_updates(
                            source.target,
                            &[CellUpdate::new(handle, replacement.clone())],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let statement = if source.target == "Flag" {
                        "UPDATE Items SET Flag=False WHERE Id=1"
                    } else {
                        "UPDATE Items SET Score=17 WHERE Id=1"
                    };
                    let query = parse_query(statement, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected, "{}", identity.id());
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table(source.table, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), rows);
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 36);
}

#[test]
fn empty_manual_groups_commit_but_empty_readonly_or_indexed_id_groups_refuse() {
    let mut empty_saves = 0;
    let mut refused = 0;
    for header in [5, 6] {
        for identity in [
            Case::CounterPrimary,
            Case::VariablePrimary,
            Case::NullableOrdinary,
        ] {
            let source = source(header, identity);
            for typed in [true, false] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table(source.table, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    editor
                        .plan_updates(source.target, &[], &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let statement = if source.target == "Flag" {
                        "UPDATE Items SET Flag=False WHERE Id=-1"
                    } else {
                        "UPDATE Items SET Score=17 WHERE Id=-1"
                    };
                    let query = parse_query(statement, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (0, 0));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.changed_rows(), Some(0));
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                locked(&path);
                empty_saves += 1;

                let failure = if typed {
                    let editor = session
                        .edit_table(source.table, &mut budget(), &|| false)
                        .unwrap();
                    editor
                        .plan_updates("Id", &[], &mut budget(), &|| false)
                        .err()
                        .unwrap()
                } else {
                    let query =
                        parse_query("UPDATE Items SET Id=7 WHERE Id=-1", ParseLimits::default())
                            .unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .err()
                        .unwrap()
                };
                let expected = if source.last_counter.is_some() {
                    WriteBlocker::Column
                } else {
                    WriteBlocker::IndexedColumn
                };
                assert!(matches!(
                    adapter_error(&failure),
                    journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(blocker)))
                        if *blocker == expected
                ));
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.state(), SessionState::Ready);
                assert_eq!(failure.last_completed(), Some(receipt));
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.pair_state(), None);
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                refused += 1;
            }
        }
    }
    assert_eq!(empty_saves, 12);
    assert_eq!(refused, 12);
}

#[test]
fn stale_handles_preserve_receipts_and_retained_indexes_across_successive_saves() {
    for header in [5, 6] {
        let source = source(header, Case::AliasHistory);
        let first_expected = literal_after(&source, &Value::Int32(17));
        let second_expected = literal_after(&source, &Value::Int32(19));
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table(source.table, &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, first_receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), first_expected);
        let editor = session
            .edit_table(source.table, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.snapshot().rows()[0][1], Value::Int32(17));
        let failure = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(19))],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            adapter_error(&failure),
            journal::Error::Native(NativeEditError::Snapshot(SnapshotError::StaleRow))
        ));
        assert_eq!(failure.last_completed(), Some(first_receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(fs::read(&path).unwrap(), first_expected);
        assert!(!bundle.exists());
        locked(&path);
        let editor = failure
            .into_session()
            .edit_table(source.table, &mut budget(), &|| false)
            .unwrap();
        let fresh = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(fresh, Value::Int32(19))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), second_expected);
        assert!(!bundle.exists());
        locked(&path);
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
