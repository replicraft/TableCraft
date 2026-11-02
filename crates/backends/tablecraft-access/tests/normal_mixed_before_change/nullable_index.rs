//! Existing mixed-session child: unchanged nullable index keeps legacy writes.
use super::{SCOPE, Transport, budget, locked};
use std::fs;
use tablecraft_access::editing::{
    journal,
    offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState},
    sql,
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, journal::RecoveryOutcome};
use tablecraft_query::UpdateError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_nullable_index_before_change as model;

fn null_record(bytes: &[u8]) -> (usize, usize) {
    let page = 8 * 4096;
    let start = u16::from_le_bytes(bytes[page + 18..page + 20].try_into().unwrap());
    let end = u16::from_le_bytes(bytes[page + 16..page + 18].try_into().unwrap());
    assert_eq!(start & 0xe000, 0);
    (page + usize::from(start), page + usize::from(end))
}

#[test]
fn nullable_unchanged_index_keeps_four_typed_and_sql_unindexed_score_saves() {
    let mut saved = 0;
    for header in [5, 6] {
        let case = model::source(header);
        assert_eq!(case.rows[2][0], Value::Null);
        let (start, end) = null_record(&case.before);
        assert_eq!(&case.before[start + 2..start + 6], &3_i32.to_le_bytes());
        assert_eq!(case.before[end - 1] & 1, 0);
        let mut expected_rows = case.rows.clone();
        expected_rows[0][5] = Value::Int32(1018);
        for transport in [Transport::Typed, Transport::Sql] {
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&database, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&database, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = match transport {
                Transport::Typed => {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), case.rows);
                    let handle = editor.snapshot().row_handle(0).unwrap();
                    editor
                        .plan_updates(
                            "Score",
                            &[CellUpdate::new(handle, Value::Int32(17))],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                }
                Transport::Sql => {
                    let query = parse_query(
                        "UPDATE Items SET Score=17 WHERE Id=1",
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                }
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
            assert_eq!(fs::read(&database).unwrap(), case.before);
            assert!(!bundle.exists());
            locked(&database);
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (Some(1), Some(1))
            );
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            let actual = fs::read(&database).unwrap();
            assert_eq!(actual, case.expected);
            assert_eq!(&actual[9 * 4096..], &case.before[9 * 4096..]);
            assert_eq!(&actual[start..end], &case.before[start..end]);
            assert_eq!(&actual[start + 2..start + 6], &3_i32.to_le_bytes());
            assert_eq!(actual[end - 1] & 1, 0);
            assert!(!bundle.exists());
            locked(&database);
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            assert_eq!(editor.snapshot().rows(), expected_rows);
            assert_eq!(editor.snapshot().rows()[2][0], Value::Null);
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&database).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            saved += 1;
        }
    }
    assert_eq!(saved, 4);
}

fn indexed_null_target_refusal(failure: &Failure) -> bool {
    // Keep the root's typed retained-cause check: transparent wrappers may skip
    // intermediate errors in std::error::Error::source.
    let error = match failure.cause() {
        Cause::Editing(error) => error,
        Cause::Planning(sql::RecoverableError::Plan(UpdateError::Backend(error))) => error,
        _ => return false,
    };
    matches!(
        error,
        journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::IndexedColumn
        )))
    )
}

#[test]
fn nullable_indexed_id_still_refuses_transitions_without_creating_a_bundle() {
    for header in [5, 6] {
        let case = model::source(header);
        assert_eq!(case.rows[2][0], Value::Null);
        for transport in [Transport::Typed, Transport::Sql] {
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            fs::write(&database, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&database, SCOPE, &mut budget(), &|| false).unwrap();
            let result = match transport {
                Transport::Typed => {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    let handle = editor.snapshot().row_handle(2).unwrap();
                    editor.plan_updates(
                        "Id",
                        &[CellUpdate::new(handle, Value::Int32(7))],
                        &mut budget(),
                        &|| false,
                    )
                }
                Transport::Sql => {
                    let query = parse_query(
                        "UPDATE Items SET Id=7 WHERE Score=3000",
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session.plan_update(query.query(), &mut budget(), None, &|| false)
                }
            };
            let failure = result
                .err()
                .expect("the fixed profile does not admit indexed NULL transitions");
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.state(), SessionState::Ready);
            assert!(
                indexed_null_target_refusal(&failure),
                "{transport:?}: {failure:?}"
            );
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(fs::read(&database).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&database);
            failure.into_session().release().unwrap();
        }
    }
}
