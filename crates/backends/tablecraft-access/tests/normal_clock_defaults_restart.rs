#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]
//! Independently framed cold bundles reach actual active/retired TCNA5 dispatch.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};
use tablecraft_access::editing::offline::{
    CompletionKind, OfflineWriteSession, SessionState, VerifiedImage,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        DurableStorage, RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
        prepare_expected,
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::normal_clock_defaults::{self as fixtures, Recipe};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn wire(case: &fixtures::Case<'_>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(case.before);
    case.intent_wire(digest.finalize().into())
}

#[test]
#[ignore = "requires clock source18; cold replay has no provider argument or new clock sample"]
fn active_partial_and_retired_bundles_replay_original_clock_input_through_the_retained_host() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut contexts = 0;
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::LeapNoon)
    {
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, case.before).unwrap();
                let target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
                let mut pair = OfflineJournalPair::create(
                    target,
                    &bundle,
                    wire(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                {
                    let (mut target, mut journal) = pair.storage().unwrap();
                    let prepared = prepare_expected(
                        &mut target,
                        &mut journal,
                        case.before,
                        case.after,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        prepared.rollback(&mut budget(), &|| false).unwrap();
                    } else {
                        drop(prepared);
                    }
                }
                if !committed && !retired {
                    let (mut target, _) = pair.storage().unwrap();
                    target
                        .seek(SeekFrom::Start((7 * 4096 + 16) as u64))
                        .unwrap();
                    target
                        .write_all(&case.after[7 * 4096 + 16..7 * 4096 + 20])
                        .unwrap();
                    target.sync_storage().unwrap();
                    let mixed = fs::read(&path).unwrap();
                    assert_ne!(mixed, case.before);
                    assert_ne!(mixed, case.after);
                }
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let selected = if retired {
                    pair.retire(outcome).unwrap();
                    let directories = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| path.is_dir())
                        .collect::<Vec<_>>();
                    assert_eq!(directories.len(), 1);
                    directories.into_iter().next().unwrap()
                } else {
                    bundle.clone()
                };
                pair.release().unwrap();
                drop(pair);
                let session = OfflineWriteSession::open(
                    &path,
                    CatalogScope::new(255),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let (session, receipt) = if retired {
                    session
                        .confirm_retired(&selected, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    session
                        .recover_update(&selected, &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(session.state(), SessionState::Ready);
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), None);
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
                    assert_eq!(receipt.outcome(), Some(outcome));
                    assert_eq!(receipt.verified_image(), None);
                }
                assert_eq!(
                    fs::read(&path).unwrap().as_slice(),
                    if committed { case.after } else { case.before }
                );
                assert!(!selected.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 16);
}
