//! Child of normal_mixed_before_change; actual held sessions, frozen images.
use super::{SCOPE, Transport, budget, locked};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, Failure, OfflineWriteSession, Phase, SessionState},
    sql,
};
use tablecraft_ace::{EditError, RecoverableEditError, WriteBlocker};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, EditableTable, SnapshotError, UpdateProfile};
use tablecraft_io::{ExclusiveFile, journal::RecoveryOutcome};
use tablecraft_query::UpdateError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::{
    canonical_mixed_before_change::Update,
    canonical_nullable_index_id_before_change as id_model,
    canonical_nullable_key_before_change::{self as score_model, Recipe},
};

struct Intent<'a> {
    profile: UpdateProfile,
    column: &'a str,
    update: &'a Update,
    sql: String,
    null_sql: &'a str,
}

fn adapter_error(failure: &Failure) -> &journal::Error {
    match failure.cause() {
        Cause::Editing(error) => error,
        Cause::Planning(sql::RecoverableError::Plan(UpdateError::Backend(error))) => error,
        cause => panic!("expected retained typed editing failure, got {cause:?}"),
    }
}

fn assert_save(
    before: &[u8],
    expected: &[u8],
    rows: &[Vec<Value>],
    final_rows: &[Vec<Value>],
    intent: &Intent<'_>,
    transport: Transport,
    boundaries: bool,
) {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("target.accdb");
    let bundle = root.path().join("journal");
    fs::write(&database, before).unwrap();
    let mut work = budget();
    let session = OfflineWriteSession::open(&database, SCOPE, &mut work, &|| false).unwrap();
    let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
    assert_eq!(editor.update_profile(), intent.profile);
    assert_eq!(editor.snapshot().rows(), rows);
    let old = editor.snapshot().row_handle(intent.update.ordinal).unwrap();
    assert_eq!(
        editor.snapshot().resolve(old).unwrap(),
        rows[intent.update.ordinal]
    );

    let staged = match transport {
        Transport::Typed => editor.plan_updates(
            intent.column,
            &[CellUpdate::new(
                old,
                Value::Int32(intent.update.replacement),
            )],
            &mut work,
            &|| false,
        ),
        Transport::Sql => {
            let query = parse_query(&intent.sql, ParseLimits::default()).unwrap();
            editor
                .into_session()
                .plan_update(query.query(), &mut work, None, &|| false)
        }
    }
    .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
    assert_eq!(fs::read(&database).unwrap(), before);
    assert!(!bundle.exists());
    locked(&database);

    let (mut session, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (Some(1), Some(1))
    );
    assert_eq!(session.state(), SessionState::Ready);
    assert_eq!(session.last_completed(), Some(receipt));
    assert_eq!(fs::read(&database).unwrap(), expected);
    assert!(!bundle.exists());
    locked(&database);

    let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
    assert_eq!(editor.update_profile(), intent.profile);
    assert_eq!(editor.snapshot().rows(), final_rows);
    session = editor.into_session();

    if boundaries {
        let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
        assert_eq!(editor.snapshot().resolve(old), Err(SnapshotError::StaleRow));
        let failure = editor
            .plan_updates(
                intent.column,
                &[CellUpdate::new(old, Value::Int32(42))],
                &mut work,
                &|| false,
            )
            .err()
            .expect("a prior snapshot handle cannot address the refreshed image");
        assert!(matches!(
            adapter_error(&failure),
            journal::Error::Native(NativeEditError::Snapshot(SnapshotError::StaleRow))
        ));
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read(&database).unwrap(), expected);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        locked(&database);
        session = failure.into_session();

        let result = match transport {
            Transport::Typed => {
                let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
                let column = editor
                    .snapshot()
                    .columns()
                    .iter()
                    .position(|column| column.name() == intent.column)
                    .unwrap();
                assert_eq!(editor.snapshot().rows()[2][column], Value::Null);
                let handle = editor.snapshot().row_handle(2).unwrap();
                editor.plan_updates(
                    intent.column,
                    &[CellUpdate::new(handle, Value::Int32(42))],
                    &mut work,
                    &|| false,
                )
            }
            Transport::Sql => {
                let query = parse_query(intent.null_sql, ParseLimits::default()).unwrap();
                session.plan_update(query.query(), &mut work, None, &|| false)
            }
        };
        let failure = result
            .err()
            .expect("indexed NULL transitions remain refused");
        let refused = if intent.profile == UpdateProfile::StoredScalar {
            matches!(
                adapter_error(&failure),
                journal::Error::Plan(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::IndexedColumn
                )))
            )
        } else {
            matches!(
                adapter_error(&failure),
                journal::Error::Native(NativeEditError::Unsupported)
            )
        };
        assert!(refused, "{failure:?}");
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(fs::read(&database).unwrap(), expected);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
        locked(&database);
        session = failure.into_session();
    }

    session.release().unwrap();
    ExclusiveFile::open(&database).unwrap().close().unwrap();
    assert_eq!(fs::read(&database).unwrap(), expected);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn nullable_score_keys_publish_twenty_four_typed_and_sql_literal_images() {
    let mut saved = 0;
    for header in [5, 6] {
        for recipe in score_model::RECIPES {
            let case = score_model::source(header, recipe);
            for transport in [Transport::Typed, Transport::Sql] {
                let intent = Intent {
                    profile: UpdateProfile::ExistingInt32,
                    column: "Score",
                    update: &case.update,
                    sql: format!(
                        "UPDATE Items SET Score={} WHERE Id=1",
                        case.update.replacement
                    ),
                    null_sql: "UPDATE Items SET Score=42 WHERE Id=3",
                };
                assert_save(
                    &case.before,
                    &case.expected,
                    &case.rows,
                    &case.final_rows,
                    &intent,
                    transport,
                    recipe == Recipe::ScalarAscending,
                );
                // Every NULL row retains its literal inactive payload and
                // complete physical record, not only its decoded NULL cell.
                for &ordinal in &case.null_rows {
                    let word = 8 * 4096 + 14 + 2 * ordinal;
                    let start = 8 * 4096
                        + usize::from(u16::from_le_bytes(
                            case.before[word..word + 2].try_into().unwrap(),
                        ));
                    let end = 8 * 4096
                        + usize::from(u16::from_le_bytes(
                            case.before[word - 2..word].try_into().unwrap(),
                        ));
                    assert_eq!(
                        &case.before[start + 6..start + 10],
                        &((ordinal as i32 + 1) * 1000).to_le_bytes()
                    );
                    assert_eq!(case.before[end - 1] & 0x20, 0);
                    assert_eq!(&case.expected[start..end], &case.before[start..end]);
                }
                assert_eq!(
                    &case.expected[9 * 4096..10 * 4096],
                    &case.before[9 * 4096..10 * 4096]
                );
                saved += 1;
            }
        }
    }
    assert_eq!(saved, 24);
}

#[test]
fn nullable_id_companion_publishes_four_present_id_and_macro_score_images() {
    let mut saved = 0;
    for header in [5, 6] {
        let case = id_model::source(header);
        assert_eq!(case.rows[2][0], Value::Null);
        assert_eq!(case.final_rows[0][0], Value::Int32(7));
        assert_eq!(case.final_rows[0][5], Value::Int32(2007));
        for transport in [Transport::Typed, Transport::Sql] {
            let intent = Intent {
                profile: UpdateProfile::StoredScalar,
                column: case.column,
                update: &case.update,
                sql: "UPDATE Items SET Id=7 WHERE Score=1000".to_owned(),
                null_sql: "UPDATE Items SET Id=42 WHERE Score=3000",
            };
            assert_save(
                &case.before,
                &case.expected,
                &case.rows,
                &case.final_rows,
                &intent,
                transport,
                true,
            );
            let start_word = 8 * 4096 + 18;
            let start = 8 * 4096
                + usize::from(u16::from_le_bytes(
                    case.before[start_word..start_word + 2].try_into().unwrap(),
                ));
            let end = 8 * 4096
                + usize::from(u16::from_le_bytes(
                    case.before[start_word - 2..start_word].try_into().unwrap(),
                ));
            assert_eq!(&case.before[start + 2..start + 6], &3_i32.to_le_bytes());
            assert_eq!(case.before[end - 1] & 1, 0);
            assert_eq!(&case.expected[start..end], &case.before[start..end]);
            saved += 1;
        }
    }
    assert_eq!(saved, 4);
}
