#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! The actual consuming batch trait keeps ordered omissions and one retained lease.
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::{
    NativeEditError, journal,
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{
    CellUpdate, EditableTable, InsertValue, OmittingInsertableBatch, SnapshotError,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_increment_multi as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(242);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], wanted: &str) {
    assert_eq!(
        Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        wanted
    );
}

fn rows() -> Vec<Vec<InsertValue>> {
    fixtures::presence()
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| value.map_or(InsertValue::Omitted, InsertValue::Supplied))
                .collect()
        })
        .collect()
}

#[test]
#[ignore = "requires all four native/SDK-qualified multi-index Increment views"]
fn atomic_retained_batch_has_honest_receipts_fresh_handles_and_one_exclusive_lease() {
    let mut saves = 0;
    for case in fixtures::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table(case.table, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.snapshot().rows(), case.rows(false));
        let stale = editor.snapshot().row_handle(0).unwrap();
        let session = editor.into_session();
        let staged = session
            .insert_table(case.table, &mut budget(), &|| false)
            .unwrap()
            .plan_insert_rows_with_omissions(rows(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.inserted_rows(), 2);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(2));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));

        let editor = session
            .edit_table(case.table, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(editor.snapshot().rows(), case.rows(true));
        let failure = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(stale, Value::Int32(8))],
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
        assert_eq!(failure.last_completed(), Some(receipt));
        let session = failure.into_session();

        // Shape validation fails on the later row, retaining the same session
        // without publishing any prefix of the first row.
        let mut malformed = rows();
        malformed[1].pop();
        let failure = session
            .insert_table(case.table, &mut budget(), &|| false)
            .unwrap()
            .plan_insert_rows_with_omissions(malformed, &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Edit(
                tablecraft_ace::EditError::Unsupported(tablecraft_ace::WriteBlocker::Column)
            )))
        ));
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let session = failure.into_session();

        let staged = session
            .insert_table(case.table, &mut budget(), &|| false)
            .unwrap()
            .plan_insert_rows_with_omissions(Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.inserted_rows(), 0);
        let (session, empty) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(empty.changed_rows(), Some(0));
        assert_eq!(empty.matched_rows(), None);
        assert_eq!(session.last_completed(), Some(empty));
        assert_eq!(fs::read(&path).unwrap(), case.after);
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 2);
}

#[test]
#[ignore = "requires qualified two-row TCNA2/V1 source images and active-retired restart"]
fn multi_index_increment_cold_restarts_use_original_omission_tcna2_and_checked_v1() {
    use tablecraft_access::editing::offline::{CompletionKind, VerifiedImage};
    use tablecraft_io::journal::{
        filesystem::{OfflineJournalPair, OfflineTarget},
        prepare_expected,
    };
    let mut contexts = 0;
    for case in fixtures::load_all(verify) {
        let mut digest = Sha256::new();
        digest.update(b"TCNI0001:BEFORE\0");
        digest.update((case.before.len() as u64).to_le_bytes());
        digest.update(&case.before);
        let binding: [u8; 32] = digest.finalize()[..].try_into().unwrap();
        let wire = case.wire(binding, false);
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
                    let (mut target, mut log) = pair.storage().unwrap();
                    let prepared = prepare_expected(
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
    assert_eq!(contexts, 8);
}
