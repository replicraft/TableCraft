use super::{
    Cause, CompletionKind, KnownOutcome, OfflineWriteSession, Phase, Receipt, SessionState,
    VerifiedImage,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_ace::{
    Int32Update, NativeImage, NativeInt32EditIntent, qualify_jet4_int32_recovery,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    IoError, SharedFile,
    journal::{RecoveryOutcome, filesystem::OfflineJournalPair, inspect_v1},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn replacement() -> Vec<u8> {
    let mut expected = mutation_fixture::database();
    let offset = 12 * 4096 + 4091;
    assert_eq!(&expected[offset..offset + 4], &10_i32.to_le_bytes());
    expected[offset..offset + 4].copy_from_slice(&12_i32.to_le_bytes());
    expected
}

fn create_retired(root: &Path, committed: bool) -> PathBuf {
    let database = root.join("target.mdb");
    let original = mutation_fixture::database();
    fs::write(&database, &original).unwrap();
    let mut owner =
        OfflineWriteSession::open(&database, CatalogScope::new(1), &mut budget(), &|| false)
            .unwrap();
    let slot = owner.resources.first_mut().unwrap();
    let image = NativeImage::capture(
        slot.target.as_mut().unwrap(),
        slot.scope,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 12)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), original);
    assert_eq!(plan.after_bytes(), replacement());
    let intent = plan.intent().encode(&mut budget(), &|| false).unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &root.join("active"),
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let outcome = {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
            RecoveryOutcome::Committed
        } else {
            let outcome = prepared.rollback(&mut budget(), &|| false).unwrap();
            assert_eq!(outcome, RecoveryOutcome::RolledBack);
            outcome
        }
    };
    pair.retire(outcome).unwrap();
    pair.release().unwrap();
    database
}

#[test]
fn confirmed_images_keep_observation_only_diagnostics_after_cleanup_failure() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let database = create_retired(root.path(), committed);
        let retired = fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.is_dir())
            .expect("the actual retired bundle must remain intact");
        let mut owner =
            OfflineWriteSession::open(&database, CatalogScope::new(1), &mut budget(), &|| false)
                .unwrap();
        let slot = owner.resources.first_mut().unwrap();
        let target = slot.target.take().unwrap();
        let pair = OfflineJournalPair::open_retired(
            target,
            &retired,
            NativeInt32EditIntent::MAX_ENCODED_BYTES as u64,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        slot.pair = Some(pair);
        slot.state = SessionState::RetainedJournal;
        let outcome = {
            let slot = owner.resources.first_mut().unwrap();
            let pair = slot.pair.as_mut().unwrap();
            let intent =
                NativeInt32EditIntent::decode(pair.intent_bytes(), &mut budget(), &|| false)
                    .unwrap();
            let (mut target, mut journal) = pair.confirmation_storage().unwrap();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified =
                qualify_jet4_int32_recovery(checked, intent, slot.scope, &mut budget(), &|| false)
                    .unwrap();
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap()
        };
        let image = if committed {
            assert_eq!(outcome, RecoveryOutcome::Committed);
            VerifiedImage::Replacement
        } else {
            assert_eq!(outcome, RecoveryOutcome::RolledBack);
            VerifiedImage::Original
        };
        // The native confirmation above is real and write-denied. Only now add
        // foreign garbage to cause a later namespace cleanup failure.
        fs::write(retired.join("foreign"), b"retain this file").unwrap();
        let failure = owner
            .finish(
                Receipt {
                    kind: CompletionKind::Confirmed,
                    known: KnownOutcome::Verified(image),
                    matched: None,
                    changed: None,
                },
                outcome,
                true,
            )
            .err()
            .expect("foreign garbage must prevent directory removal");
        assert_eq!(failure.phase(), Phase::Finalization);
        assert_eq!(failure.state(), SessionState::RetainedJournal);
        assert!(matches!(failure.cause(), Cause::Finalization(_)));
        assert_eq!(
            failure.current_outcome(),
            Some(KnownOutcome::Verified(image))
        );
        assert_eq!(failure.last_completed(), None);
        let diagnostic = failure.to_string();
        assert!(diagnostic.contains("Verified"));
        assert!(diagnostic.contains(if committed { "Replacement" } else { "Original" }));
        assert!(!diagnostic.contains("Committed"), "{diagnostic}");
        assert!(!diagnostic.contains("RolledBack"), "{diagnostic}");
        assert_eq!(
            fs::read(&database).unwrap(),
            if committed {
                replacement()
            } else {
                mutation_fixture::database()
            }
        );
        assert_eq!(
            fs::read(retired.join("foreign")).unwrap(),
            b"retain this file"
        );
        assert!(matches!(
            SharedFile::open(&database),
            Err(IoError::LockBusy)
        ));
        failure.into_session().release().unwrap();
    }
}
