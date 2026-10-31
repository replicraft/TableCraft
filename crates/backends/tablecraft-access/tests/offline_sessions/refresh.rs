use super::{budget, model, session, write_original};
use std::fs;
use tablecraft_access::editing::offline::{
    Cause, OfflineWriteSession, OpenError, Phase, SessionState,
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{IoError, SharedFile};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn completed_receipt_survives_capture_failure_until_explicit_refresh() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let retained = session(&database, &mut budget());
    let first = parse_query(
        "UPDATE Items SET Score=12 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let staged = retained
        .plan_update(first.query(), &mut budget(), None, &|| false)
        .unwrap();
    let (retained, completed) = staged
        .apply(&root.path().join("first"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert_eq!(retained.last_completed(), Some(completed));

    let next = parse_query(
        "UPDATE Items SET Score=Score+5 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let mut no_bytes = ResourceBudget::new(0, 1_000_000, 1_000_000_000);
    let failure = retained
        .plan_update(next.query(), &mut no_bytes, None, &|| false)
        .err()
        .expect("the new source capture must exhaust its byte allowance");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(
        failure.cause(),
        Cause::Native(InspectError::Limit(_))
    ));
    assert_eq!(failure.state(), SessionState::RefreshRequired);
    assert_eq!(failure.last_completed(), Some(completed));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));

    let failure = failure
        .into_session()
        .plan_update(next.query(), &mut budget(), None, &|| false)
        .err()
        .expect("a fresh budget cannot bypass the required explicit refresh");
    assert!(matches!(failure.cause(), Cause::InvalidState));
    assert_eq!(failure.last_completed(), Some(completed));
    assert_eq!(failure.state(), SessionState::RefreshRequired);

    let failure = failure
        .into_session()
        .refresh(&mut budget(), &|| true)
        .err()
        .expect("cancelled refresh must retain the target and receipt");
    assert_eq!(failure.phase(), Phase::Refresh);
    assert!(matches!(
        failure.cause(),
        Cause::Native(InspectError::Cancelled)
    ));
    assert_eq!(failure.state(), SessionState::RefreshRequired);
    assert_eq!(failure.last_completed(), Some(completed));
    assert_eq!(fs::read(&database).unwrap(), model::with_score(12));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));

    let retained = failure
        .into_session()
        .refresh(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(retained.state(), SessionState::Ready);
    assert_eq!(retained.last_completed(), Some(completed));
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    let staged = retained
        .plan_update(next.query(), &mut budget(), None, &|| false)
        .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
    let (retained, next_completed) = staged
        .apply(&root.path().join("second"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fs::read(&database).unwrap(), model::with_score(17));
    assert_eq!(retained.last_completed(), Some(next_completed));
    assert_eq!(retained.current_outcome(), None);
    assert!(matches!(
        SharedFile::open(&database),
        Err(IoError::LockBusy)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    retained.release().unwrap();
    SharedFile::open(&database).unwrap().close().unwrap();
}

#[test]
fn zero_byte_open_refuses_before_attempting_an_exclusive_lock() {
    let root = tempfile::tempdir().unwrap();
    let database = write_original(root.path());
    let shared = SharedFile::open(&database).unwrap();
    let mut no_bytes = ResourceBudget::new(0, 1_000_000, 1_000_000_000);
    let failure =
        OfflineWriteSession::open(&database, CatalogScope::new(1), &mut no_bytes, &|| false)
            .err()
            .expect("retained-owner allocation must be paid before target lock acquisition");
    assert!(matches!(failure, OpenError::Native(InspectError::Limit(_))));
    // This held shared lease would cause LockBusy if acquisition were reached.
    shared.close().unwrap();
    let retained = session(&database, &mut budget());
    assert_eq!(fs::read(&database).unwrap(), model::original());
    retained.release().unwrap();
}
