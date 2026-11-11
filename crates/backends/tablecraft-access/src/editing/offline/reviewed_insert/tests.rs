//! Exact selected INSERT families retain original requests and captured defaults.
use super::*;
use crate::editing::offline::test_support;
use std::{
    fs,
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::{InsertFieldValue, NativeImage, OmittedRowInsert, RowInsert};
use tablecraft_core::{
    limits::Resource,
    values::{ExtendedDateTime, Value},
};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_io::{IoError, SharedFile};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_defaults as defaults, canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(259);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap()
}

#[derive(Clone, Copy)]
enum Family {
    CompleteTyped,
    CompleteTypedGrowing,
    Complete,
    Omitted,
    AllocateEof,
}

fn review(family: Family) -> (Vec<u8>, NativeJournalInsertPlan, Option<Vec<u8>>) {
    match family {
        Family::CompleteTyped | Family::Complete => {
            let source = defaults::source(defaults::Case::Absent);
            let (expected, _) = defaults::inserted(&source, false).unwrap();
            let rows = vec![RowInsert::new(vec![Value::Int32(24), Value::Null])];
            let plan = match family {
                Family::CompleteTyped => NativeJournalInsertPlan::CompleteTyped(
                    image(&source.bytes)
                        .plan_recoverable_ordinary_complete_row_inserts(
                            "Items",
                            rows,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap(),
                ),
                Family::Complete => NativeJournalInsertPlan::Complete(
                    image(&source.bytes)
                        .plan_recoverable_ordinary_row_inserts(
                            "Items",
                            rows,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap(),
                ),
                _ => unreachable!(),
            };
            (source.bytes, plan, Some(expected))
        }
        Family::Omitted => {
            let source = defaults::source(defaults::Case::Seven);
            let (expected, _) = defaults::inserted(&source, false).unwrap();
            let rows = vec![OmittedRowInsert::new(vec![
                InsertFieldValue::Supplied(Value::Int32(24)),
                InsertFieldValue::Omitted,
            ])];
            let plan = image(&source.bytes)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    rows,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            (
                source.bytes,
                NativeJournalInsertPlan::Omitted(plan),
                Some(expected),
            )
        }
        Family::AllocateEof | Family::CompleteTypedGrowing => {
            let source = defaults::slot_saturated_source();
            let expected = defaults::slot_saturated_inserted(&source);
            let rows = vec![RowInsert::new(vec![Value::Int32(256), Value::Int32(2560)])];
            let plan = match family {
                Family::AllocateEof => NativeJournalInsertPlan::AllocateEof(
                    image(&source)
                        .plan_recoverable_ordinary_eof_row_inserts(
                            "Items",
                            rows,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap(),
                ),
                Family::CompleteTypedGrowing => NativeJournalInsertPlan::CompleteTyped(
                    image(&source)
                        .plan_recoverable_ordinary_complete_row_inserts(
                            "Items",
                            rows,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap(),
                ),
                _ => unreachable!(),
            };
            assert!(plan.after_bytes().len() > source.len());
            (source, plan, Some(expected))
        }
    }
}

#[test]
fn exact_existing_and_growing_families_requalify_and_apply_without_reselection() {
    for family in [
        Family::CompleteTyped,
        Family::CompleteTypedGrowing,
        Family::Complete,
        Family::Omitted,
        Family::AllocateEof,
    ] {
        let (before, reviewed, independent) = review(family);
        let wire = reviewed.encode_intent(&mut budget(), &|| false).unwrap();
        let after = reviewed.after_bytes().to_vec();
        if let Some(expected) = independent {
            assert!(
                after == expected,
                "independent complete INSERT image differs"
            );
        }
        let reviewed = reviewed
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            reviewed.encode_intent(&mut budget(), &|| false).unwrap(),
            wire
        );
        assert!(
            reviewed.after_bytes() == after,
            "requalified INSERT image differs"
        );
        assert_eq!(reviewed.inserted_rows(), 1);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("insert.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let staged = session
            .stage_reviewed_insert_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        test_support::assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(1));
        test_support::assert_completion(&receipt);
        session.release().unwrap();
        assert!(
            fs::read(&path).unwrap() == after,
            "committed INSERT image differs"
        );
    }
}

struct OnceClock {
    calls: AtomicUsize,
}

impl Clock for OnceClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.calls.fetch_add(1, Ordering::Relaxed) != 0 {
            return Err(EvalError::ClockUnavailable);
        }
        ClockReading::new(ExtendedDateTime::new(1899, 12, 30, 12, 0, 0, 0).unwrap())
    }
}

#[test]
fn clock_family_keeps_one_captured_reading_and_original_presence_through_save() {
    let before = defaults::clock_source();
    let expected = defaults::clock_inserted(&before, 24, 0.5_f64.to_bits());
    let clock = OnceClock {
        calls: AtomicUsize::new(0),
    };
    let rows = vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Supplied(Value::Int32(24)),
        InsertFieldValue::Omitted,
    ])];
    let plan = image(&before)
        .plan_recoverable_ordinary_clock_row_inserts("Items", rows, &clock, &mut budget(), &|| {
            false
        })
        .unwrap();
    assert!(plan.intent().reading().is_some());
    let reviewed = NativeJournalInsertPlan::Clock(plan);
    let wire = reviewed.encode_intent(&mut budget(), &|| false).unwrap();
    let reviewed = reviewed
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        reviewed.encode_intent(&mut budget(), &|| false).unwrap(),
        wire
    );
    assert!(
        reviewed.after_bytes() == expected,
        "retained clock INSERT image differs"
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clock.accdb");
    fs::write(&path, &before).unwrap();
    let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
    let staged = session
        .stage_reviewed_insert_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
        .unwrap();
    let (session, _) = staged
        .apply(&directory.path().join("journal"), &mut budget(), &|| false)
        .unwrap();
    session.release().unwrap();
    assert_eq!(clock.calls.load(Ordering::Relaxed), 1);
    assert!(
        fs::read(path).unwrap() == expected,
        "committed clock INSERT image differs"
    );
}

#[test]
fn stale_capture_cancellation_and_each_pool_exhaustion_retain_same_owner() {
    for case in 0..4 {
        let (before, reviewed, _) = review(Family::CompleteTyped);
        let mut current = before;
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
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
            .stage_reviewed_insert_with_budgets(reviewed, &mut native, &mut proof, &|| case == 3)
            .err()
            .expect("must refuse before journal");
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert_eq!(
            failure.state(),
            if matches!(case, 1 | 3) {
                SessionState::RefreshRequired
            } else {
                SessionState::Ready
            }
        );
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert!(
            fs::read(path).unwrap() == current,
            "refused INSERT modified source"
        );
    }
}

#[test]
fn proof_and_native_work_are_both_consumed_for_empty_original_requests() {
    let before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let plan = image(&before)
        .plan_recoverable_ordinary_complete_row_inserts("Items", vec![], &mut budget(), &|| false)
        .unwrap();
    let mut native = budget();
    let mut proof = budget();
    let result = NativeJournalInsertPlan::CompleteTyped(plan)
        .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
        .unwrap();
    assert!(
        result.after_bytes() == before,
        "empty INSERT modified source"
    );
    assert_eq!(result.inserted_rows(), 0);
    assert!(native.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
    assert!(proof.remaining(Resource::Steps) < budget().remaining(Resource::Steps));
}

#[cfg(feature = "ace16-create")]
mod opaque;

#[cfg(feature = "ace16-create")]
mod keyless;
