//! Actual TCIC2 stage/recovery is distinct from the still-working original route.
use super::*;
use crate::editing::offline::{
    CompletionKind,
    plan::MutationIntent,
    test_support::{assert_staged_read_state, retained_schema_recovery::cold_case},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, RecoverableNativeFreshIndexCreatePlan, SecondaryIndexCreate};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, history};
const SCOPE: CatalogScope = CatalogScope::new(28622);
fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(case: &raw::Source) -> RecoverableNativeFreshIndexCreatePlan {
    NativeImage::capture_retained(
        &mut Cursor::new(&case.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_retained_fresh_long_index_creation(
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
fn fresh_index_stage_selects_its_original_and_applies_only_a_schema_receipt() {
    for descending in [false, true] {
        let case = history::source_with_direction(descending);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fresh.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let staged = open(&path)
            .stage_reviewed_fresh_index_creation_with_budgets(
                plan(&case),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let encoded = staged.plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert!(encoded.starts_with(b"TCIC0002"));
        assert!(matches!(
            MutationIntent::decode(&encoded, &mut budget(), &|| false),
            Ok(MutationIntent::FreshIndexCreation(_))
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
        same(&fs::read(&path).unwrap(), &case.expected);
    }
}

#[test]
fn fresh_index_cold_pending_committed_and_settled_retain_its_schema_kind_and_complete_image() {
    let case = history::source();
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
