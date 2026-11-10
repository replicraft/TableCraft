//! Real retained schema actions preserve V3 ownership and schema-only receipts.
use super::open;
use crate::editing::offline::{
    CompletionKind, Receipt, SchemaChange, VerifiedImage,
    test_support::{
        assert_completion, assert_staged_read_state,
        saved_field::{FieldPlan, retain_retained_pair, retired_namespace},
    },
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, RecoverableFieldDropPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes as changes,
    fields::{Case, Target},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn review(before: &[u8], target: Target) -> RecoverableFieldDropPlan {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(1610),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_retained_tail_field_drop("Table1", target.name(), &mut budget(), &|| false)
    .unwrap()
}

fn schema_receipt(receipt: &Receipt, committed: bool) {
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        committed.then_some(SchemaChange::ColumnDropped)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
    assert_completion(receipt);
}

#[test]
fn retained_schema_review_has_no_namespace_and_apply_publishes_exact_native_images() {
    for (case, target) in [
        (Case::FirstNumber, Target::Number),
        (Case::SecondText, Target::ShortText),
    ] {
        let source = changes::source(case, target, true, true, true);
        let expected = changes::drop_tail(
            &source.bytes,
            source.definition,
            source.catalog_root,
            target.name(),
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retained.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let staged = open(&path)
            .stage_reviewed_field_drop_with_budgets(
                review(&source.bytes, target),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_staged_read_state(&path, &source.bytes);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        schema_receipt(&receipt, true);
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
    }
}

#[test]
fn retained_schema_cold_and_settled_v3_recovery_report_only_actual_action_effects() {
    let target = Target::ShortText;
    let source = changes::source(Case::SecondText, target, true, true, true);
    let expected = changes::drop_tail(
        &source.bytes,
        source.definition,
        source.catalog_root,
        target.name(),
    );
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let owner = retain_retained_pair(
                open(&path),
                &bundle,
                FieldPlan::Drop(review(&source.bytes, target)),
                committed,
                retired,
                &mut budget(),
            );
            owner.release().unwrap();
            let (owner, receipt) = if retired {
                open(&path)
                    .confirm_retired(&retired_namespace(directory.path()), &mut budget(), &|| {
                        false
                    })
                    .unwrap()
            } else {
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            };
            schema_receipt(&receipt, committed);
            if retired {
                assert_eq!(receipt.kind(), CompletionKind::Confirmed);
                assert_eq!(
                    receipt.verified_image(),
                    Some(if committed {
                        VerifiedImage::Replacement
                    } else {
                        VerifiedImage::Original
                    })
                );
            } else {
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
            }
            owner.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed { &expected } else { &source.bytes }.as_slice()
            );
        }
    }
}
