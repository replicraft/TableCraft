//! Real original14 staging and stateful V3 recovery use independent native AFTER bytes.
use super::*;
use crate::editing::offline::CompletionKind;
use std::{fs, io::Cursor};
use tablecraft_ace::{CreateColumn, UnadornedFieldAdditionDefinition};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, typed_fields::Kind, unadorned_fields as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(17313);
fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}
fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}
fn plan(before: &[u8], text: bool) -> RecoverableUnadornedFieldAdditionPlan {
    let image =
        NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
            .unwrap();
    let definition = UnadornedFieldAdditionDefinition::capture(
        if text {
            CreateColumn::short_text("Amount", 37).unwrap()
        } else {
            CreateColumn::new("Amount", ScalarType::Currency)
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    image
        .plan_recoverable_retained_unadorned_field_addition(
            "Table1",
            definition,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

fn root_order(bytes: &mut [u8]) {
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + index] = [9, 4, 0, 1][index] ^ mask;
    }
}
#[test]
fn unadorned_generated_currency_stage_apply_is_schema_only_and_matches_independent_complete_image()
{
    let mut source = fields::source(fields::Case::FirstNumber);
    root_order(&mut source.bytes);
    let expected = raw::after(
        &source.bytes,
        source.definition,
        "Amount",
        Kind::Currency,
        [9, 4, 0, 1],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("declared.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.bytes).unwrap();
    let staged = open(&path)
        .stage_reviewed_retained_unadorned_field_addition_with_budgets(
            plan(&source.bytes, false),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(&fs::read(&path).unwrap(), &source.bytes);
    assert!(!bundle.exists());
    let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    owner.release().unwrap();
    same(&fs::read(&path).unwrap(), &expected);
}

#[test]
fn unadorned_parameterized_text_cold_pending_committed_and_settled_rebuild_original14() {
    let mut source = fields::source(fields::Case::FirstNumber);
    root_order(&mut source.bytes);
    let expected = raw::after(
        &source.bytes,
        source.definition,
        "Amount",
        Kind::ShortText {
            units: 37,
            compression: false,
        },
        [9, 4, 0, 1],
    );
    for committed in [false, true] {
        for retired in [false, true] {
            let original = plan(&source.bytes, true);
            let encoded = original.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(encoded.starts_with(b"TCAC0014"));
            crate::editing::offline::test_support::retained_schema_recovery::cold_case(
                &source.bytes,
                &expected,
                encoded,
                committed,
                retired,
                SchemaChange::ColumnAdded,
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
fn unadorned_stage_allocation_and_cancellation_refusals_keep_exclusive_owner_before_namespace() {
    let source = fields::source(fields::Case::FirstNumber);
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("declared.accdb");
        fs::write(&path, &source.bytes).unwrap();
        let mut native = ResourceBudget::new(0, 0, 0);
        let mut proof = budget();
        let failure = open(&path)
            .stage_reviewed_retained_unadorned_field_addition_with_budgets(
                plan(&source.bytes, false),
                &mut native,
                &mut proof,
                &|| cancelled,
            )
            .err()
            .expect("expected refusal before allocating stage box");
        assert_eq!(failure.phase(), Phase::Planning);
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
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
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        same(&fs::read(&path).unwrap(), &source.bytes);
        failure.into_session().release().unwrap();
    }
}

#[test]
fn unadorned_stage_rejects_a_valid_changed_schema_and_retains_the_same_target_owner() {
    let source = fields::source(fields::Case::FirstNumber);
    let reviewed = plan(&source.bytes, false);
    // Independently author another valid declaration before opening the target.
    // It is not the original image the review authorized.
    let changed = raw::after(
        &source.bytes,
        source.definition,
        "OtherValue",
        Kind::Currency,
        [9, 4, 0, 1],
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("changed.accdb");
    fs::write(&path, &changed).unwrap();
    let failure = open(&path)
        .stage_reviewed_retained_unadorned_field_addition_with_budgets(
            reviewed,
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("a valid replacement schema cannot replace the reviewed original");
    assert_eq!(failure.phase(), Phase::Planning);
    assert!(matches!(failure.cause(), Cause::SourceChanged));
    assert!(failure.pair_state().is_none());
    assert!(failure.current_outcome().is_none());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    same(&fs::read(&path).unwrap(), &changed);
    failure.into_session().release().unwrap();
}
