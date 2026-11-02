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
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable, SnapshotError};
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
    let cases = sources::load_all(verify);
    let mut commits = 0;
    let mut stale = 0;
    for case in cases {
        for sql in [false, true] {
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
                session
                    .insert_table(case.table, &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(case.values(), &mut budget(), &|| false)
                    .unwrap()
            };
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
                let mut expected = case.rows(true);
                if alias == "Descending" {
                    expected.rotate_right(1);
                }
                assert_eq!(read.rows(), expected);
                let count = tablecraft_ace::inspect_table_index_counter(
                    &mut Cursor::new(fs::read(&path).unwrap()),
                    SCOPE,
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(count.value(), if alias == "PrimaryID" { 401 } else { 2 });
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
    assert_eq!((commits, stale), (8, 8));
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
    for case in sources::load_all(verify) {
        let mut digest = Sha256::new();
        digest.update(b"TCNI0001:BEFORE\0");
        digest.update((case.before.len() as u64).to_le_bytes());
        digest.update(&case.before);
        let binding: [u8; 32] = digest.finalize()[..].try_into().unwrap();
        let wire = case.wire(binding, 401, i64::MAX, 6817);
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
    assert_eq!(contexts, 16);
}
