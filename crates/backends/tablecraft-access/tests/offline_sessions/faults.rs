use super::{budget, model, session, write_original};
use std::{
    cell::{Cell, RefCell},
    fs::{self, File, OpenOptions},
    os::unix::fs::FileExt,
};
use tablecraft_access::editing::offline::{Cause, KnownOutcome, Phase};
use tablecraft_io::{
    IoError, SharedFile,
    journal::{JournalError, RecoveryOutcome, filesystem::PairState},
};
use tablecraft_sql::{ParseLimits, parse_query};

const SCORE_OFFSET: u64 = 12 * 4096 + 4091;

#[test]
fn interrupted_commit_recovers_the_same_retained_pair_without_unlocking() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let observer = File::open(&database).unwrap();
    let retained = session(&database, &mut budget());
    let query = parse_query(
        "UPDATE Items SET Score=12 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = retained
        .plan_update(query.query(), &mut budget(), None, &|| false)
        .unwrap();
    let observed_replacement = Cell::new(false);
    let observer_failure = RefCell::new(None);
    let cancelled = || {
        let mut bytes = [0; 4];
        if let Err(error) = observer.read_exact_at(&mut bytes, SCORE_OFFSET) {
            *observer_failure.borrow_mut() = Some(error);
            return true;
        }
        if bytes == 12_i32.to_le_bytes() {
            observed_replacement.set(true);
            return true;
        }
        false
    };
    let bundle = root.path().join("active");
    let failure = staged
        .apply(&bundle, &mut budget(), &cancelled)
        .err()
        .expect("commit must observe cancellation after writing the target");
    assert!(observer_failure.borrow().is_none());
    assert!(observed_replacement.get());
    assert_eq!(failure.phase(), Phase::Commit);
    assert!(!failure.invalid_input());
    assert!(matches!(failure.cause(), Cause::Commit(_)));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.pair_state(), Some(PairState::RecoveryRequired));
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));

    let (retained, receipt) = failure
        .into_session()
        .recover_pending(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::RolledBack));
    assert_eq!(fs::read(&database).unwrap(), model::original());
    assert!(!bundle.exists());
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let query = parse_query(
        "UPDATE Items SET Score=17 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = retained
        .plan_update(query.query(), &mut budget(), None, &|| false)
        .unwrap();
    let (retained, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(fs::read(&database).unwrap(), model::with_score(17));
    retained.release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}

#[test]
fn cleanup_failure_preserves_known_commit_and_blocks_replay() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let observer = File::open(&database).unwrap();
    let retained = session(&database, &mut budget());
    let query = parse_query(
        "UPDATE Items SET Score=12 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = retained
        .plan_update(query.query(), &mut budget(), None, &|| false)
        .unwrap();
    let bundle = root.path().join("active");
    let inserted = Cell::new(false);
    let insertion_failure = RefCell::new(None);
    let observer_failure = RefCell::new(None);
    // Insert foreign garbage only after the target replacement is observable,
    // independently of setup validation. No owned journal, intent or binding
    // byte is replaced or forged by this deliberate namespace fault.
    let cancelled = || {
        let mut bytes = [0; 4];
        if let Err(error) = observer.read_exact_at(&mut bytes, SCORE_OFFSET) {
            *observer_failure.borrow_mut() = Some(error);
            return true;
        }
        if !inserted.get() && bytes == 12_i32.to_le_bytes() {
            match fs::write(bundle.join("foreign"), b"retain this file") {
                Ok(()) => inserted.set(true),
                Err(error) => *insertion_failure.borrow_mut() = Some(error),
            }
        }
        false
    };
    let failure = staged
        .apply(&bundle, &mut budget(), &cancelled)
        .err()
        .expect("foreign garbage must block final directory cleanup");
    assert!(observer_failure.borrow().is_none());
    assert!(insertion_failure.borrow().is_none());
    assert!(inserted.get());
    assert_eq!(failure.phase(), Phase::Finalization);
    assert_eq!(
        failure.current_outcome(),
        Some(KnownOutcome::Native(RecoveryOutcome::Committed))
    );
    assert_eq!(
        failure.pair_state(),
        Some(PairState::RetiredGarbageUncertain)
    );
    assert_eq!(
        failure.pair_namespace_state(),
        Some(PairState::RetiredGarbageUncertain)
    );
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert!(!bundle.exists());
    let retired = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.is_dir())
        .expect("partly cleaned retired directory must remain");
    assert_eq!(
        fs::read(retired.join("foreign")).unwrap(),
        b"retain this file"
    );
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));

    let failure = failure
        .into_session()
        .recover_pending(&mut budget(), &|| false)
        .err()
        .expect("known completion must never replay recovery");
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert!(!failure.invalid_input());
    let failure = failure
        .into_session()
        .plan_update(query.query(), &mut budget(), None, &|| false)
        .err()
        .expect("partly cleaned pair must not permit a fresh update");
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert!(!failure.invalid_input());
    let failure = failure
        .into_session()
        .refresh(&mut budget(), &|| false)
        .err()
        .expect("refresh must not bypass retained garbage");
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert!(!failure.invalid_input());
    assert_eq!(
        failure.current_outcome(),
        Some(KnownOutcome::Native(RecoveryOutcome::Committed))
    );
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert_eq!(
        fs::read(retired.join("foreign")).unwrap(),
        b"retain this file"
    );
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    failure.into_session().release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}

#[test]
fn changed_target_after_planning_preserves_third_image_and_empty_journal() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let retained = session(&database, &mut budget());
    let query = parse_query(
        "UPDATE Items SET Score=12 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = retained
        .plan_update(query.query(), &mut budget(), None, &|| false)
        .unwrap();
    // An explicitly uncooperative test writer changes the source revision while
    // the real session remains held. This is not a supported concurrent editor.
    let outsider = OpenOptions::new().write(true).open(&database).unwrap();
    outsider
        .write_all_at(&17_i32.to_le_bytes(), SCORE_OFFSET)
        .unwrap();
    outsider.sync_all().unwrap();
    let bundle = root.path().join("active");
    let failure = staged
        .apply(&bundle, &mut budget(), &|| false)
        .err()
        .expect("source precondition must reject the third image");
    assert_eq!(failure.phase(), Phase::Prepare);
    assert!(!failure.invalid_input());
    assert!(matches!(
        failure.cause(),
        Cause::Prepare(JournalError::TargetMismatch)
    ));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(fs::read(&database).unwrap(), model::with_score(17));
    assert_eq!(fs::metadata(bundle.join("journal")).unwrap().len(), 0);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let failure = failure
        .into_session()
        .recover_pending(&mut budget(), &|| false)
        .err()
        .expect("empty journal has no recovery authority");
    assert_eq!(failure.phase(), Phase::Recovery);
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(fs::read(&database).unwrap(), model::with_score(17));
    assert_eq!(fs::metadata(bundle.join("journal")).unwrap().len(), 0);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    failure.into_session().release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}
