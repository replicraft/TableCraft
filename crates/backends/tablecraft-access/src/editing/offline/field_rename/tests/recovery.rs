//! Retained/cold/settled recovery shares the existing pair and receipt lifecycle.
use super::*;
use std::path::PathBuf;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};

fn retained(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    committed: bool,
    retired: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let plan = review(before, "Score", "Total");
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(intent.starts_with(b"TCFR0001"));
    let mut owner = open(path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner
}

fn retired_bundle(directory: &Path) -> PathBuf {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
        });
    let path = entries.next().expect("retained retired journal namespace");
    assert!(entries.next().is_none());
    path
}

#[test]
fn original_field_request_recovers_retained_and_cold_v1_pairs() {
    let before = raw::source(raw::SourceCase::Number);
    let after = review(&before, "Score", "Total").after_bytes().to_vec();
    for committed in [false, true] {
        for cold in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained(&path, &bundle, &before, committed, false);
            let (owner, receipt) = if cold {
                owner.release().unwrap();
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            } else {
                owner.recover_pending(&mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.kind(), CompletionKind::Recovered);
            assert_eq!(
                receipt.outcome(),
                Some(if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                })
            );
            assert_receipt(&receipt, committed);
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(
                fs::read(path).unwrap(),
                if committed { &after } else { &before }.as_slice()
            );
        }
    }
}

#[test]
fn settled_confirmation_verifies_current_image_without_inventing_historical_outcome() {
    let before = raw::source(raw::SourceCase::Number);
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("confirm.accdb");
        let bundle = directory.path().join("journal");
        retained(&path, &bundle, &before, committed, true)
            .release()
            .unwrap();
        let retired = retired_bundle(directory.path());
        let expected = fs::read(&path).unwrap();
        let (owner, receipt) = open(&path)
            .confirm_retired(&retired, &mut budget(), &|| false)
            .unwrap();
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
        assert_receipt(&receipt, committed);
        owner.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}
