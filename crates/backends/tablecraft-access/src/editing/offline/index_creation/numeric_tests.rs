//! Numeric original source binding, schema-only publication and actual cold recovery.
use super::*;
use crate::editing::offline::{
    CompletionKind,
    plan::MutationIntent,
    test_support::{assert_staged_read_state, retained_schema_recovery::cold_case},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, RecoverableNativeNumericIndexCreatePlan, SecondaryIndexCreate};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::numeric::{
    self as raw, Kind, Population,
};

const SCOPE: CatalogScope = CatalogScope::new(28623);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(case: &raw::Source) -> RecoverableNativeNumericIndexCreatePlan {
    NativeImage::capture_retained(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_retained_numeric_index_creation(
        case.table,
        &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
        &mut budget(),
        &|| false,
    )
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
fn numeric_stage_preserves_original_kind_and_applies_only_a_schema_receipt() {
    for (kind, descending) in [
        (Kind::Boolean, false),
        (Kind::Currency, true),
        (Kind::Double, false),
    ] {
        let case = raw::source(kind, Population::NullableDuplicate, descending);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("numeric.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let staged = open(&path)
            .stage_reviewed_numeric_index_creation_with_budgets(
                plan(&case),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let encoded = staged.plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert!(encoded.starts_with(b"TCIC0003"));
        assert!(matches!(
            MutationIntent::decode(&encoded, &mut budget(), &|| false),
            Ok(MutationIntent::NumericIndexCreation(_))
        ));
        assert_staged_read_state(&path, &case.before);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.schema_change(), Some(SchemaChange::IndexCreated));
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        owner.release().unwrap();
        same(&fs::read(path).unwrap(), &case.expected);
    }
}

#[test]
fn numeric_cold_pending_committed_and_settled_dispatch_reconstructs_complete_images() {
    let case = raw::source(Kind::Int64, Population::NullableDuplicate, true);
    for committed in [false, true] {
        for retired in [false, true] {
            let original = plan(&case);
            let encoded = original.intent().encode(&mut budget(), &|| false).unwrap();
            cold_case(
                &case.before,
                &case.expected,
                encoded,
                committed,
                retired,
                SchemaChange::IndexCreated,
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
fn numeric_stage_stale_source_and_cancellation_retain_owner_without_a_journal() {
    for cancelled in [false, true] {
        let case = raw::source(Kind::Currency, Population::NullableDuplicate, false);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("numeric.accdb");
        let actual = if cancelled {
            &case.before
        } else {
            &case.expected
        };
        fs::write(&path, actual).unwrap();
        let failed = open(&path).stage_reviewed_numeric_index_creation_with_budgets(
            plan(&case),
            &mut budget(),
            &mut budget(),
            &|| cancelled,
        );
        let failure = match failed {
            Err(failure) => failure,
            Ok(_) => panic!("stale or cancelled numeric review must refuse"),
        };
        let owner = failure.into_session();
        owner.release().unwrap();
        same(&fs::read(path).unwrap(), actual);
        assert!(!directory.path().join("journal").exists());
    }
}
