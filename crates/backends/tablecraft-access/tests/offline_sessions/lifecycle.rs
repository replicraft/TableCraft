use super::{budget, model, session, write_original};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_access::editing::offline::{
    Cause, CompletionKind, KnownOutcome, SessionState, VerifiedImage,
};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

fn assert_locked(database: &Path) {
    assert!(matches!(
        ExclusiveFile::open(database),
        Err(IoError::LockBusy)
    ));
}

#[test]
fn successive_normal_edits_keep_one_lease_and_refresh_original_expectations() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let mut allowance = budget();
    let mut owner = session(&database, &mut allowance);
    for (score, bundle) in [(12, "first"), (17, "second")] {
        let query = parse_query(
            &format!("UPDATE Items SET Score={score} WHERE ID=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let staged = owner
            .plan_update(query.query(), &mut allowance, None, &|| false)
            .unwrap();
        assert_eq!(staged.matched_rows(), 1);
        assert_eq!(staged.changed_rows(), 1);
        assert_locked(&database);
        let before = if score == 12 {
            model::original()
        } else {
            model::with_score(12)
        };
        assert_eq!(fs::read(&database).unwrap(), before);
        assert!(!root.path().join(bundle).exists());

        let (next, receipt) = staged
            .apply(&root.path().join(bundle), &mut allowance, &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.verified_image(), None);
        assert_eq!(next.state(), SessionState::Ready);
        assert_eq!(next.last_completed(), Some(receipt));
        assert_eq!(next.current_outcome(), None);
        assert_locked(&database);
        assert_eq!(fs::read(&database).unwrap(), model::with_score(score));
        assert!(!root.path().join(bundle).exists());
        owner = next;
    }
    owner.release().unwrap();
    ExclusiveFile::open(&database).unwrap().close().unwrap();
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn abandoning_a_staged_plan_and_committing_a_noop_preserve_the_original() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let mut allowance = budget();
    let owner = session(&database, &mut allowance);
    let query = parse_query(
        "UPDATE Items SET Score=12 WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = owner
        .plan_update(query.query(), &mut allowance, None, &|| false)
        .unwrap();
    let owner = staged.into_session();
    assert_locked(&database);
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);

    let query = parse_query(
        "UPDATE Items SET Score=Score WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = owner
        .plan_update(query.query(), &mut allowance, None, &|| false)
        .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 0));
    let (owner, receipt) = staged
        .apply(&root.path().join("noop"), &mut allowance, &|| false)
        .unwrap();
    assert_eq!(receipt.changed_rows(), Some(0));
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert_locked(&database);
    owner.release().unwrap();
}

#[test]
fn closed_sql_and_ace_normal_write_refusals_keep_targets_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let mut allowance = budget();
    let owner = session(&database, &mut allowance);
    let query = parse_query(
        "UPDATE Items SET Score=12,ID=4 WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let failure = owner
        .plan_update(query.query(), &mut allowance, None, &|| false)
        .err()
        .expect("closed normal SQL shape");
    assert!(failure.invalid_input());
    assert!(matches!(failure.cause(), Cause::Planning(_)));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.last_completed(), None);
    assert_locked(&database);
    assert_eq!(fs::read(&database).unwrap(), model::original());
    failure.into_session().release().unwrap();

    // Existing public independently qualified ACE source owner; no header patch.
    let case = tablecraft_testkit::access::secondary_index::source(
        6,
        tablecraft_testkit::access::secondary_index::Population::Distinct,
        false,
    );
    let ace = root.path().join("ordinary.accdb");
    fs::write(&ace, &case.before).unwrap();
    let owner = session(&ace, &mut allowance);
    let query = parse_query(
        "UPDATE Items SET Score=12 WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let failure = owner
        .plan_update(query.query(), &mut allowance, None, &|| false)
        .err()
        .expect("ACE normal profile stays unavailable");
    assert!(matches!(failure.cause(), Cause::Planning(_)));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(fs::read(&ace).unwrap(), case.before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    failure.into_session().release().unwrap();
}

fn retained_pair(root: &Path, committed: bool, retired: bool) -> (PathBuf, PathBuf) {
    let database = write_original(root);
    let active = root.join("active");
    let mut allowance = budget();
    let mut target = OfflineTarget::open(&database, &mut allowance, &|| false).unwrap();
    let image =
        NativeImage::capture(&mut target, CatalogScope::new(1), &mut allowance, &|| false).unwrap();
    let plan = image
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 12)],
            &mut allowance,
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), model::original());
    assert_eq!(plan.after_bytes(), model::with_score(12));
    let intent = plan.intent().encode(&mut allowance, &|| false).unwrap();
    let mut pair =
        OfflineJournalPair::create(target, &active, intent, &mut allowance, &|| false).unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut allowance, &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut allowance, &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut allowance, &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        } else {
            // An active pending fixture intentionally retains interrupted work.
            drop(prepared);
        }
    }
    let bundle = if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
        fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".tablecraft-retired-")
            })
            .unwrap()
    } else {
        active
    };
    pair.release().unwrap();
    drop(pair);
    (database, bundle)
}

#[test]
fn real_pending_and_committed_recovery_return_the_still_locked_session() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (database, bundle) = retained_pair(root.path(), committed, false);
        let mut allowance = budget();
        let owner = session(&database, &mut allowance);
        let (owner, receipt) = owner
            .recover_update(&bundle, &mut allowance, &|| false)
            .unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(
            receipt.outcome(),
            Some(if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            })
        );
        assert_eq!(receipt.verified_image(), None);
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(
            fs::read(&database).unwrap(),
            if committed {
                model::with_score(12)
            } else {
                model::original()
            }
        );
        assert!(!bundle.exists());
        assert_locked(&database);
        assert_eq!(owner.last_completed(), Some(receipt));
        owner.release().unwrap();
    }
}

#[test]
fn retired_confirmation_reports_current_images_without_claiming_history() {
    for committed in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (database, bundle) = retained_pair(root.path(), committed, true);
        let before = fs::read(&database).unwrap();
        let mut allowance = budget();
        let owner = session(&database, &mut allowance);
        let (owner, receipt) = owner
            .confirm_retired(&bundle, &mut allowance, &|| false)
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
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(fs::read(&database).unwrap(), before);
        assert!(!bundle.exists());
        assert_locked(&database);
        owner.release().unwrap();
    }
}

#[test]
fn byte_identical_wrong_target_refuses_pairing_and_cannot_resume_normal_use() {
    let root = tempfile::tempdir().unwrap();
    let (database, bundle) = retained_pair(root.path(), false, false);
    let wrong = root.path().join("copy.mdb");
    fs::write(&wrong, model::original()).unwrap();
    let journal = fs::read(bundle.join("journal")).unwrap();
    let mut allowance = budget();
    let owner = session(&wrong, &mut allowance);
    let failure = owner
        .recover_update(&bundle, &mut allowance, &|| false)
        .err()
        .expect("different target object");
    assert!(matches!(failure.cause(), Cause::Setup));
    assert!(failure.setup_failure().is_some());
    assert!(!failure.invalid_input());
    assert_eq!(failure.current_outcome(), None);
    assert_locked(&wrong);
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert_eq!(fs::read(&wrong).unwrap(), model::original());
    assert_eq!(fs::read(bundle.join("journal")).unwrap(), journal);

    let query = parse_query(
        "UPDATE Items SET Score=17 WHERE ID=1",
        ParseLimits::default(),
    )
    .unwrap();
    let failure = failure
        .into_session()
        .plan_update(query.query(), &mut allowance, None, &|| false)
        .err()
        .expect("retained setup blocks normal use");
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert!(!failure.invalid_input());
    assert_eq!(failure.current_outcome(), None::<KnownOutcome>);
}
