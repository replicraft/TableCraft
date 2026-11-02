#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{
    CompletionKind, OfflineWriteSession, SessionState, VerifiedImage,
};
use tablecraft_ace::{NativeImage, RowInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ordinary_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(160);

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

#[test]
#[ignore = "requires all four pinned ordinary INSERT SOURCE images"]
fn ordinary_insert_sessions_recover_active_and_confirm_retired_original_intents() {
    let cases = sources::load_all(verify);
    let mut contexts = 0;
    for case in cases {
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
                let image =
                    NativeImage::capture(&mut target, SCOPE, &mut budget(), &|| false).unwrap();
                let plan = image
                    .plan_recoverable_ordinary_row_inserts(
                        "Sentinel",
                        vec![RowInsert::new(sources::values())],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.before_bytes(), case.before);
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.inserted_rows(), 1);
                let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
                assert_eq!(&wire[..8], b"TCNA0001");
                let mut pair =
                    OfflineJournalPair::create(target, &bundle, wire, &mut budget(), &|| false)
                        .unwrap();
                {
                    let (mut target, mut journal) = pair.storage().unwrap();
                    let prepared = plan
                        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap();
                    if committed {
                        assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
                    } else if retired {
                        assert_eq!(
                            prepared.rollback(&mut budget(), &|| false).unwrap(),
                            RecoveryOutcome::RolledBack
                        );
                    } else {
                        drop(prepared);
                    }
                }
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
                    assert_eq!(receipt.verified_image(), None);
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
                assert_eq!(
                    fs::read(&path).unwrap().as_slice(),
                    if committed {
                        case.after.as_slice()
                    } else {
                        case.before.as_slice()
                    }
                );
                assert!(!selected.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                session.release().unwrap();
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 8);
}
