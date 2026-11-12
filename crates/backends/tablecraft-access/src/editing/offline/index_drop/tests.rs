//! Actual offline DROP keeps original identity, exclusive resources and V3 receipts.
use super::*;
use crate::editing::offline::{
    Cause, CompletionKind,
    plan::MutationIntent,
    test_support::{assert_staged_read_state, retained_schema_recovery::cold_case},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, SecondaryIndexCreate};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, dropping};

const SCOPE: CatalogScope = CatalogScope::new(28620);
fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(bytes: &[u8], case: &raw::Source) -> RecoverableNativeIndexDropPlan {
    image(bytes)
        .plan_recoverable_retained_index_drop(case.table, case.index_name, &mut budget(), &|| false)
        .unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn index_drop_stage_apply_preserves_rows_and_acknowledges_only_the_schema_effect() {
    let case = raw::protected_companion(false);
    let expected = dropping::after(&case);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("drop.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &case.expected).unwrap();
    let staged = open(&path)
        .stage_reviewed_index_drop_with_budgets(
            plan(&case.expected, &case),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_staged_read_state(&path, &case.expected);
    assert!(!bundle.exists());
    assert_eq!(staged.table(), case.table);
    assert_eq!(staged.index_name(), case.index_name);
    assert_eq!(staged.retired_pages(), case.index_pages.len() + 1);
    let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(receipt.schema_change(), Some(SchemaChange::IndexDropped));
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    owner.release().unwrap();
    same(&fs::read(&path).unwrap(), &expected);
    // A guarded fresh source still proves its original rows and primary index.
    let recreated = image(&expected)
        .prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(recreated.logical_index_number(), 1);
}

#[test]
fn index_drop_cold_pending_committed_and_settled_keep_the_schema_receipt_and_complete_image() {
    let case = raw::protected_companion(false);
    let expected = dropping::after(&case);
    for committed in [false, true] {
        for retired in [false, true] {
            let original = plan(&case.expected, &case);
            let wire = original.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(wire.starts_with(b"TCID0001"));
            assert!(matches!(
                MutationIntent::decode(&wire, &mut budget(), &|| false),
                Ok(MutationIntent::IndexDrop(_))
            ));
            cold_case(
                &case.expected,
                &expected,
                wire,
                committed,
                retired,
                SchemaChange::IndexDropped,
                |pair, committed, retired| {
                    let (mut target, mut journal) = pair.resizable_storage().unwrap();
                    let prepared = original
                        .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap();
                    if committed {
                        prepared.commit(&mut budget(), &|| false).unwrap();
                    } else if retired {
                        prepared.rollback(&mut budget(), &|| false).unwrap();
                    }
                },
            );
        }
    }
}

#[test]
fn index_drop_stale_original_and_cancelled_capture_keep_the_target_owner_without_a_namespace() {
    let case = raw::protected_companion(false);
    let alias = dropping::surviving_alias(&case, "ScoreAlias");
    for (cancelled, exhausted) in [(false, false), (true, false), (false, true)] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drop.accdb");
        let actual = if cancelled || exhausted {
            &case.expected
        } else {
            &alias.before
        };
        fs::write(&path, actual).unwrap();
        let mut native = if exhausted {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_index_drop_with_budgets(
                plan(&case.expected, &case),
                &mut native,
                &mut budget(),
                &|| cancelled,
            )
            .err()
            .expect("stale source/cancellation must refuse before namespace creation");
        assert_eq!(failure.phase(), Phase::Planning);
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        let owner = failure.into_session();
        owner.release().unwrap();
        same(&fs::read(&path).unwrap(), actual);
    }
}

#[test]
fn index_drop_family_keeps_unknown_versions_and_truncation_out_of_generic_update_decoding() {
    for raw in [
        b"TCID".as_slice(),
        b"TCID0001".as_slice(),
        b"TCID0002".as_slice(),
    ] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::IndexDropIntent(_))
        ));
    }
}
