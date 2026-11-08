//! Lease-free SQL selection is never reevaluated under the exclusive save lease.
use super::*;
use crate::editing::{offline::test_support, sql};
use std::{fs, io::Cursor};
use tablecraft_ace::read_table;
use tablecraft_core::values::Value;
use tablecraft_io::{IoError, SharedFile};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

const SCOPE: CatalogScope = CatalogScope::new(258);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn review(before: &[u8], predicate: &str) -> DeletePlan<RecoverableNativeDeletePlan> {
    let image =
        NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap();
    let parsed = parse_query(
        &format!("DELETE FROM Items WHERE {predicate}"),
        ParseLimits::default(),
    )
    .unwrap();
    sql::plan_recoverable_delete(image, parsed.query(), &mut budget(), None, &|| false).unwrap()
}

#[test]
fn reviewed_multirow_and_empty_delete_apply_sealed_candidate_and_reopen() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    for (predicate, matched) in [("Id>=63", 2), ("Id=999", 0)] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("delete.mdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let reviewed = review(&before, predicate);
        let expected = reviewed.backend().after_bytes().to_vec();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let staged = session
            .stage_reviewed_delete_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            (staged.matched_rows(), staged.deleted_rows()),
            (matched, matched)
        );
        test_support::assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(matched), Some(matched))
        );
        test_support::assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        let rows = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows().len(), 64 - matched);
        assert_eq!(rows.rows()[0], [Value::Int32(1), Value::Int32(10)]);
    }
}

#[test]
fn stale_or_exhausted_or_cancelled_review_retains_target_without_journal() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    );
    for case in 0..4 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.mdb");
        let reviewed = review(&before, "Id=2");
        let mut current = before.clone();
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        fs::write(&path, &current).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = session
            .stage_reviewed_delete_with_budgets(reviewed, &mut native, &mut proof, &|| case == 3)
            .err()
            .expect("review must refuse");
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert_eq!(failure.phase(), Phase::Planning);
        let expected_state = match case {
            0 | 2 => SessionState::Ready,
            1 | 3 => SessionState::RefreshRequired,
            _ => unreachable!("test contains four explicit failure preconditions"),
        };
        assert_eq!(failure.state(), expected_state);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(failure.pair_state(), None);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        test_support::assert_staged_read_state(&path, &current);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}
