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
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CompleteInsertableBatch, DeletableTable, EditableTable, InsertableTable, SnapshotError,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_wide_root_promotion as sources;

const SCOPE: CatalogScope = CatalogScope::new(288);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires six native and inert-SDK-qualified wider promotion source files"]
fn typed_and_explicit_scalar_sql_promotions_preserve_literal_images_leases_receipts_and_stale_handles()
 {
    let mut cases = sources::load_all(verify);
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(verify),
    );
    let mut commits = 0;
    let mut stale = 0;
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_leaf_compression::load_all(verify),
    );
    for case in &cases {
        case.assert_literal_layout();
    }
    for case in cases {
        for sql in [false, true] {
            if sql && case.inserted != 1 {
                continue;
            }
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            let snapshot = EditableTable::snapshot(&editor);
            assert_eq!(snapshot.rows(), case.rows(false));
            let old = snapshot.row_handle(0).unwrap();
            let session = editor.into_session();
            let staged = if sql {
                let query = parse_query(&case.sql(), ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .unwrap()
            } else {
                let inserter = session
                    .insert_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                if case.inserted == 1 {
                    inserter
                        .plan_insert(case.values(), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    inserter
                        .plan_insert_rows(case.insert_values(), &mut budget(), &|| false)
                        .unwrap()
                }
            };
            assert_eq!(staged.inserted_rows(), case.inserted);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(case.inserted));
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            let snapshot = EditableTable::snapshot(&editor);
            assert_eq!(snapshot.rows(), case.rows(true));
            assert_eq!(snapshot.resolve(old), Err(SnapshotError::StaleRow));
            for alias in ["PrimaryID", "Ascending", "Descending", "Alias"] {
                let read = tablecraft_ace::read_table_index(
                    &mut Cursor::new(fs::read(&path).unwrap()),
                    SCOPE,
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(read.rows(), case.index_rows(alias));
                let count = tablecraft_ace::inspect_table_index_counter(
                    &mut Cursor::new(fs::read(&path).unwrap()),
                    SCOPE,
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    count.value(),
                    if alias == "PrimaryID" {
                        400 + case.inserted as u32
                    } else {
                        1 + case.inserted as u32
                    }
                );
            }
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
            failure.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            commits += 1;
            stale += 1;
        }
    }
    assert_eq!((commits, stale), (16, 16));
}

#[test]
#[ignore = "requires four literal promotion pairs and TCNA4/V2 active-retired restart"]
fn wider_promotion_cold_restarts_use_actual_tcna4_v2_host_recovery_and_retired_confirmation() {
    use tablecraft_access::editing::offline::{CompletionKind, VerifiedImage};
    use tablecraft_io::journal::{
        filesystem::{OfflineJournalPair, OfflineTarget},
        prepare_resizable_expected,
    };
    let mut contexts = 0;
    let mut cases = sources::load_all(verify);
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(verify),
    );
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_leaf_compression::load_all(verify),
    );
    for case in &cases {
        case.assert_literal_layout();
    }
    for case in cases {
        let mut digest = Sha256::new();
        digest.update(b"TCNI0001:BEFORE\0");
        digest.update((case.before.len() as u64).to_le_bytes());
        digest.update(&case.before);
        let binding: [u8; 32] = digest.finalize()[..].try_into().unwrap();
        let wire = case.wire_rows(binding, 401);
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
                let mut pair = OfflineJournalPair::create(
                    target,
                    &bundle,
                    wire.clone(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                {
                    let (mut target, mut log) = pair.resizable_storage().unwrap();
                    let prepared = prepare_resizable_expected(
                        &mut target,
                        &mut log,
                        &case.before,
                        &case.after,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        assert_eq!(
                            prepared.rollback(&mut budget(), &|| false).unwrap(),
                            RecoveryOutcome::RolledBack
                        );
                    } else {
                        drop(prepared);
                    }
                }
                let expected = if committed { &case.after } else { &case.before };
                let selected = if retired {
                    pair.retire(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                    .unwrap();
                    let dirs: Vec<_> = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| path.is_dir())
                        .collect();
                    assert_eq!(dirs.len(), 1);
                    dirs.into_iter().next().unwrap()
                } else {
                    bundle.clone()
                };
                pair.release().unwrap();
                drop(pair);
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let (session, receipt) = if retired {
                    session
                        .confirm_retired(&selected, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    session
                        .recover_update(&selected, &mut budget(), &|| false)
                        .unwrap()
                };
                if retired {
                    assert_eq!(receipt.kind(), CompletionKind::Confirmed);
                    assert_eq!(receipt.outcome(), None);
                    assert_eq!(
                        receipt.verified_image(),
                        Some(if committed {
                            VerifiedImage::Replacement
                        } else {
                            VerifiedImage::Original
                        })
                    );
                } else {
                    assert_eq!(receipt.kind(), CompletionKind::Recovered);
                    assert_eq!(
                        receipt.outcome(),
                        Some(if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        })
                    );
                }
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), None);
                assert_eq!(session.state(), SessionState::Ready);
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), *expected);
                assert!(!selected.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session.release().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 48);
}

#[test]
#[ignore = "requires qualified two-row prefix sources and actual atomic batch service"]
fn complete_batches_fail_atomically_and_sql_batch_syntax_stays_closed() {
    use tablecraft_access::editing::sql::RecoverableInsertError;
    use tablecraft_ace::{EditError, IndexConstraintViolation, InspectError, RecoverableEditError};
    use tablecraft_query::QueryError;
    let cases =
        tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(verify);
    let mut contexts = 0;
    for case in cases {
        for control in [0, 1, 2] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let failure = if control == 2 {
                let query =
                    parse_query(&case.unsupported_batch_sql(), ParseLimits::default()).unwrap();
                session
                    .plan_insert(query.query(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            } else {
                let mut rows = case.insert_values();
                if control == 0 {
                    rows[1][0] = tablecraft_core::values::Value::Int32(1);
                }
                let inserter = session
                    .insert_table(case.table, &mut budget(), &|| false)
                    .unwrap();
                if control == 1 {
                    inserter
                        .plan_insert_rows(rows, &mut budget(), &|| true)
                        .err()
                        .unwrap()
                } else {
                    inserter
                        .plan_insert_rows(rows, &mut budget(), &|| false)
                        .err()
                        .unwrap()
                }
            };
            match control {
                0 => assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Plan(RecoverableEditError::Edit(
                        EditError::Inspect(InspectError::IndexConstraint(
                            IndexConstraintViolation::DuplicateKey
                        ))
                    )))
                )),
                1 => assert!(matches!(
                    failure.cause(),
                    Cause::Editing(journal::Error::Native(NativeEditError::Inspect(
                        InspectError::Cancelled
                    )))
                )),
                2 => assert!(matches!(
                    failure.cause(),
                    Cause::InsertPlanning(RecoverableInsertError::Shape(QueryError::Unsupported))
                )),
                _ => unreachable!(),
            }
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(failure.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            failure.into_session().release().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 12);
}
