#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    fs::{self, File},
};
use tablecraft_access::editing::offline::{
    Cause, CompletionKind, OfflineWriteSession, Phase, SessionState, VerifiedImage,
};
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeUpdateIntent,
    NativeUpdateProfile, RecoverableNativeEditPlan, SlotScalarUpdate,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        JournalError, RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget, PairState},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_eof_allocation as sources;

const SCOPE: CatalogScope = CatalogScope::new(153);

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

fn plan(target: &mut OfflineTarget) -> RecoverableNativeEditPlan {
    let image = NativeImage::capture(target, SCOPE, &mut budget(), &|| false).unwrap();
    let group = NativeFittedIntentGroup::new(
        "Marker",
        NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
            0,
            Value::Text(sources::OLD.into()),
            Value::Text(sources::GROWN.into()),
        )]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    image
        .plan_recoverable_ordinary_eof_updates("Sentinel", vec![group], &mut budget(), &|| false)
        .unwrap()
}

#[test]
#[ignore = "requires all four pinned EOF-allocation SOURCE images"]
fn eof_sessions_recover_active_v2_and_confirm_retired_v2_for_both_outcomes() {
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
                let plan = plan(&mut target);
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(
                    plan.intent().profile(),
                    NativeUpdateProfile::Ace16OrdinaryEofAllocation
                );
                let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
                assert_eq!(&intent[..8], b"TCNI0007");
                let mut pair =
                    OfflineJournalPair::create(target, &bundle, intent, &mut budget(), &|| false)
                        .unwrap();
                {
                    let (mut target, mut journal) = pair.resizable_storage().unwrap();
                    let prepared = plan
                        .prepare_update(&mut target, &mut journal, &mut budget(), &|| false)
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
                let expected = if committed { &case.after } else { &case.before };
                assert_eq!(fs::read(&path).unwrap(), *expected);
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
                assert_eq!(fs::read(&path).unwrap(), *expected);
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

#[test]
#[ignore = "requires all four pinned EOF-allocation SOURCE images"]
fn interrupted_eof_session_commit_recovers_pending_on_the_same_lease_and_truncates() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let observer = File::open(&path).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let sql = format!(
            "UPDATE Sentinel SET Marker='{}' WHERE [ID]=23",
            sources::GROWN
        );
        let query = parse_query(&sql, ParseLimits::default()).unwrap();
        let staged = session
            .plan_update(query.query(), &mut budget(), None, &|| false)
            .unwrap();
        let extended = Cell::new(false);
        let observer_error = RefCell::new(None);
        let cancelled = || {
            match observer.metadata() {
                Ok(metadata) => {
                    if metadata.len() > case.before.len() as u64 {
                        extended.set(true);
                        return true;
                    }
                }
                Err(error) => {
                    *observer_error.borrow_mut() = Some(error);
                    return true;
                }
            }
            false
        };
        let failure = staged
            .apply(&bundle, &mut budget(), &cancelled)
            .err()
            .unwrap();
        assert!(observer_error.borrow().is_none());
        assert!(extended.get());
        assert_eq!(failure.phase(), Phase::Commit);
        assert!(matches!(failure.cause(), Cause::Commit(_)));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(failure.last_completed(), None);
        assert_eq!(failure.pair_state(), Some(PairState::RecoveryRequired));
        let partial = fs::read(&path).unwrap();
        assert!(partial.len() > case.before.len() && partial.len() <= case.after.len());
        for (at, byte) in partial.iter().enumerate() {
            assert!(case.before.get(at) == Some(byte) || case.after.get(at) == Some(byte));
        }
        assert!(bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        let (session, receipt) = failure
            .into_session()
            .recover_pending(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::RolledBack));
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(fs::read(&path).unwrap(), case.before);
        assert!(!bundle.exists());
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        session.release().unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
#[ignore = "requires all four pinned EOF-allocation SOURCE images"]
fn owned_only_intent_with_v2_journal_refuses_without_cross_family_fallback() {
    for case in sources::load_all(verify) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("target.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let mut target = OfflineTarget::open(&path, &mut budget(), &|| false).unwrap();
        let plan = plan(&mut target);
        assert_eq!(plan.after_bytes(), case.after);
        let mut wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        wire[..8].copy_from_slice(b"TCNI0006");
        wire[12..14].copy_from_slice(&6_u16.to_le_bytes());
        let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            intent.profile(),
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
        );
        let mut pair =
            OfflineJournalPair::create(target, &bundle, wire, &mut budget(), &|| false).unwrap();
        {
            let (mut target, mut journal) = pair.resizable_storage().unwrap();
            drop(
                plan.prepare_update(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap(),
            );
        }
        let retained =
            ["binding", "intent", "journal"].map(|name| fs::read(bundle.join(name)).unwrap());
        pair.release().unwrap();
        drop(pair);
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let failure = session
            .recover_update(&bundle, &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(matches!(
            failure.cause(),
            Cause::Recovery(JournalError::InvalidJournal)
        ));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read(&path).unwrap(), case.before);
        for (name, bytes) in ["binding", "intent", "journal"].into_iter().zip(retained) {
            assert_eq!(fs::read(bundle.join(name)).unwrap(), bytes);
        }
        assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
        failure.into_session().release().unwrap();
    }
}
