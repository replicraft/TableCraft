//! Reviewed SQL candidates requalify over the same held native target.
use super::*;
use crate::editing::{offline::test_support, sql};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{InspectError, read_table};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_io::{IoError, SharedFile, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::mutation_fixture;

const SCOPE: CatalogScope = CatalogScope::new(173);
const UPDATE: &str = "UPDATE Items SET Score=Score+7 WHERE Id>=2";

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn review(bytes: &[u8], statement: &str) -> UpdatePlan<RecoverableNativeEditPlan> {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let query = parse_query(statement, ParseLimits::default()).unwrap();
    sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
        .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_locked(path: &Path) {
    assert!(matches!(SharedFile::open(path), Err(IoError::LockBusy)));
}

fn expected(before: &[u8]) -> Vec<u8> {
    let mut after = before.to_vec();
    // Independent original fixture DATA and fixed-carrier layout. The native
    // planner, encoder and production format constants do not build this oracle.
    let page = 12 * 4096;
    for (ordinal, original, replacement) in [(1, 20_i32, 27_i32), (2, 30, 37)] {
        let slot = page + 14 + 2 * ordinal;
        let start = usize::from(u16::from_le_bytes(
            before[slot..slot + 2].try_into().unwrap(),
        ));
        let score = page + start + 6;
        assert_eq!(&before[score..score + 4], &original.to_le_bytes());
        after[score..score + 4].copy_from_slice(&replacement.to_le_bytes());
    }
    after
}

#[test]
fn lease_free_multirow_review_commits_exact_literal_after_and_reopens_native_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reviewed.mdb");
    let bundle = directory.path().join("journal");
    let before = mutation_fixture::database();
    let after = expected(&before);
    fs::write(&path, &before).unwrap();
    let reviewed = review(&before, UPDATE);
    assert_eq!(reviewed.matched_rows(), 2);
    SharedFile::open(&path).unwrap().close().unwrap();
    #[cfg(unix)]
    let identity = fs::metadata(&path).unwrap();

    let staged = open(&path)
        .stage_reviewed_update(reviewed, &mut budget(), &|| false)
        .unwrap();
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (2, 2));
    assert!(!bundle.exists());
    assert_locked(&path);
    test_support::assert_staged_read_state(&path, &before);
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(receipt.matched_rows(), Some(2));
    assert_eq!(receipt.changed_rows(), Some(2));
    assert_eq!(receipt.schema_change(), None);
    assert_eq!(session.last_completed(), Some(receipt));
    test_support::assert_completion(&receipt);
    assert!(!bundle.exists());
    assert_locked(&path);
    test_support::assert_staged_read_state(&path, &after);
    session.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), after);
    #[cfg(unix)]
    {
        let current = fs::metadata(&path).unwrap();
        assert_eq!(
            (current.dev(), current.ino()),
            (identity.dev(), identity.ino())
        );
    }
    let rows = read_table(
        &mut fs::File::open(&path).unwrap(),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        rows.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(27)],
            vec![Value::Int32(3), Value::Int32(37)],
        ]
    );
    assert!(after[13 * 4096..].iter().all(|byte| *byte == 0xa7));
}

#[test]
fn unchanged_and_empty_reviews_keep_native_admission_and_exact_original_image() {
    for (statement, matched) in [
        ("UPDATE Items SET Score=Score WHERE Id=2", 1),
        ("UPDATE Items SET Score=Score+7 WHERE Id=99", 0),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("no-change.mdb");
        let bundle = directory.path().join("journal");
        let before = mutation_fixture::database();
        let reviewed = review(&before, statement);
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_update(reviewed, &mut budget(), &|| false)
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (matched, 0));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.matched_rows(), Some(matched));
        assert_eq!(receipt.changed_rows(), Some(0));
        test_support::assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
        assert!(!bundle.exists());
    }
}

#[test]
fn unrelated_protected_tail_change_refuses_before_journal_and_retains_same_lease() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stale.mdb");
    let before = mutation_fixture::database();
    let reviewed = review(&before, UPDATE);
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    fs::write(&path, &changed).unwrap();
    let failure = open(&path)
        .stage_reviewed_update(reviewed, &mut budget(), &|| false)
        .err()
        .expect("unrelated byte changes invalidate the complete reviewed source");
    assert!(matches!(failure.cause(), Cause::SourceChanged));
    assert_eq!(failure.phase(), Phase::Planning);
    assert_eq!(failure.state(), SessionState::Ready);
    assert_eq!(failure.pair_state(), None);
    assert_eq!(failure.current_outcome(), None);
    assert_locked(&path);
    test_support::assert_staged_read_state(&path, &changed);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), changed);
}

#[test]
fn cancellation_or_capture_budget_failure_requires_explicit_refresh_without_losing_resources() {
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.mdb");
        let before = mutation_fixture::database();
        let reviewed = review(&before, UPDATE);
        fs::write(&path, &before).unwrap();
        let session = open(&path);
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        let failure = session
            .stage_reviewed_update(reviewed, &mut work, &|| cancelled)
            .err()
            .expect("capture must observe cancellation and exhausted resources");
        if cancelled {
            assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Cancelled)
            ));
        } else {
            assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Limit(_))
            ));
        }
        assert_eq!(failure.state(), SessionState::RefreshRequired);
        assert_eq!(failure.pair_state(), None);
        assert_locked(&path);
        test_support::assert_staged_read_state(&path, &before);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        let session = failure
            .into_session()
            .refresh(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(session.state(), SessionState::Ready);
        session.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[test]
fn earlier_completed_receipt_survives_a_later_stale_review_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sequence.mdb");
    let before = mutation_fixture::database();
    let after = expected(&before);
    let first = review(&before, UPDATE);
    let stale = review(&before, UPDATE);
    fs::write(&path, &before).unwrap();
    let (session, receipt) = open(&path)
        .stage_reviewed_update(first, &mut budget(), &|| false)
        .unwrap()
        .apply(&directory.path().join("first"), &mut budget(), &|| false)
        .unwrap();
    let failure = session
        .stage_reviewed_update(stale, &mut budget(), &|| false)
        .err()
        .expect("a reviewed original cannot overwrite a later committed image");
    assert!(matches!(failure.cause(), Cause::SourceChanged));
    assert_eq!(failure.last_completed(), Some(receipt));
    assert_eq!(failure.current_outcome(), None);
    assert_eq!(failure.pair_state(), None);
    assert_locked(&path);
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), after);
}

#[test]
fn separate_native_and_proof_pools_commit_with_exact_complete_source_comparison_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("separate-pools.mdb");
    let before = mutation_fixture::database();
    let after = expected(&before);
    let reviewed = review(&before, UPDATE);
    fs::write(&path, &before).unwrap();
    let mut native = budget();
    // Adapter source equality visits both complete images. Native intent
    // binding and rebuilt-image proof additionally consume proof steps.
    let mut proof = ResourceBudget::new(2 * before.len() as u64, 0, 100_000_000);
    let staged = open(&path)
        .stage_reviewed_update_with_budgets(reviewed, &mut native, &mut proof, &|| false)
        .unwrap();
    assert_eq!(proof.remaining(Resource::Bytes), 0);
    assert_eq!(proof.remaining(Resource::Rows), 0);
    assert!(proof.remaining(Resource::Steps) < 100_000_000);
    assert_eq!((staged.matched_rows(), staged.changed_rows()), (2, 2));
    assert_locked(&path);
    let (session, receipt) = staged
        .apply(&directory.path().join("journal"), &mut budget(), &|| false)
        .unwrap();
    test_support::assert_completion(&receipt);
    session.release().unwrap();
    assert_eq!(fs::read(path).unwrap(), after);
}

#[test]
fn native_and_proof_refusals_retain_the_same_lease_and_distinct_capture_state() {
    for exhausted in ["native", "proof-bytes", "proof-steps"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pool-refusal.mdb");
        let before = mutation_fixture::database();
        let reviewed = review(&before, UPDATE);
        fs::write(&path, &before).unwrap();
        let mut native = if exhausted == "native" {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = match exhausted {
            "proof-bytes" => ResourceBudget::new(0, 0, 100_000_000),
            "proof-steps" => ResourceBudget::new(2 * before.len() as u64, 0, 0),
            _ => budget(),
        };
        let proof_before = [
            proof.remaining(Resource::Bytes),
            proof.remaining(Resource::Rows),
            proof.remaining(Resource::Steps),
        ];
        let failure = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut native, &mut proof, &|| false)
            .err()
            .expect("each independent pool must reject its own exhausted resource");
        let Cause::Native(InspectError::Limit(limit)) = failure.cause() else {
            panic!("expected typed capture or source-comparison resource refusal");
        };
        assert_eq!(
            failure.state(),
            if exhausted == "native" {
                SessionState::RefreshRequired
            } else {
                SessionState::Ready
            },
            "{exhausted}"
        );
        if exhausted == "proof-bytes" {
            assert_eq!(limit.resource, Resource::Bytes);
        } else {
            assert_eq!(limit.resource, Resource::Steps);
        }
        if exhausted == "native" {
            assert_eq!(
                [
                    proof.remaining(Resource::Bytes),
                    proof.remaining(Resource::Rows),
                    proof.remaining(Resource::Steps),
                ],
                proof_before,
                "failed capture cannot consume the independent proof pool"
            );
        }
        assert_eq!(failure.pair_state(), None);
        assert_eq!(failure.last_completed(), None);
        assert_locked(&path);
        test_support::assert_staged_read_state(&path, &before);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[cfg(feature = "ace16-create")]
mod opaque;

#[cfg(feature = "ace16-create")]
mod currency;

#[cfg(feature = "ace16-create")]
mod boolean;

#[cfg(feature = "ace16-create")]
mod fixed_scalars;

#[cfg(feature = "ace16-create")]
mod wide_values;

#[cfg(feature = "ace16-create")]
mod retained_recovery;

#[cfg(feature = "ace16-create")]
mod empty;
