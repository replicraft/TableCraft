#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{
    CompletionKind, OfflineWriteSession, SessionState, VerifiedImage,
};
use tablecraft_ace::{NativeEofInsertIntent, RowInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{EditableTable, InsertableTable};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
        prepare_resizable_expected,
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_eof_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(195);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "requires both source native/SDK-qualified EOF INSERT directories"]
fn real_single_row_typed_sql_allocation_preserves_literal_images_leases_and_receipts() {
    let mut saves = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| !case.batch)
    {
        for typed in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("target.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                session
                    .insert_table("Sentinel", &mut budget(), &|| false)
                    .unwrap()
                    .plan_insert(sources::values(260), &mut budget(), &|| false)
                    .unwrap()
            } else {
                let query = parse_query(
                    "INSERT INTO Sentinel ([Bytes],[Marker],[ID]) VALUES (NULL,NULL,260)",
                    ParseLimits::default(),
                )
                .unwrap();
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
                .edit_table("Sentinel", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                EditableTable::snapshot(&editor).rows(),
                sources::rows(&case, true)
            );
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
            saves += 1;
        }
        // Original omission stays TCNA2 and fixed-only; it must not be expanded
        // into a complete TCNA3 request merely because EOF is representable.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let query = parse_query(
            "INSERT INTO Sentinel ([ID]) VALUES (260)",
            ParseLimits::default(),
        )
        .unwrap();
        let failure = session
            .plan_insert(query.query(), &mut budget(), &|| false)
            .err()
            .unwrap();
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    assert_eq!(saves, 4);
}

#[test]
#[ignore = "requires both source native/SDK-qualified EOF INSERT directories"]
fn active_and_retired_tcna3_v2_restart_contexts_use_fresh_native_qualification() {
    let mut contexts = 0;
    for case in sources::load_all(verify)
        .into_iter()
        .filter(|case| !case.batch)
    {
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
                // Intent is data-only. Generic IO stores the independent full
                // source images; no native producer supplies fixture bytes.
                let intent = NativeEofInsertIntent::new(
                    "Sentinel",
                    vec![RowInsert::new(sources::values(260))],
                    &case.before,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let wire = intent.encode(&mut budget(), &|| false).unwrap();
                assert_eq!(&wire[..8], b"TCNA0003");
                let mut pair =
                    OfflineJournalPair::create(target, &bundle, wire, &mut budget(), &|| false)
                        .unwrap();
                {
                    let (mut target, mut journal) = pair.resizable_storage().unwrap();
                    let prepared = prepare_resizable_expected(
                        &mut target,
                        &mut journal,
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
                    let directories = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|entry| entry.is_dir())
                        .collect::<Vec<_>>();
                    assert_eq!(directories.len(), 1);
                    directories.into_iter().next().unwrap()
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
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(session.state(), SessionState::Ready);
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
