//! Original TCFD5 requests stage and recover exact non-tail schema effects.
use super::open;
use crate::editing::offline::{
    CompletionKind, Receipt, SchemaChange,
    plan::MutationIntent,
    test_support::{
        assert_completion, assert_staged_read_state,
        saved_field::{FieldPlan, assert_retained_schema_recovery},
    },
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, RecoverableFieldDropPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    boolean, field_changes, fields, typed_fields,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1615);

struct Case {
    bytes: Vec<u8>,
    definition: u32,
    catalog_root: usize,
    table: &'static str,
    column: &'static str,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn cases() -> [Case; 2] {
    let number = fields::source(fields::Case::FirstNumber);
    let bytes = typed_fields::after(
        &number.bytes,
        number.definition,
        number.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let bytes = typed_fields::update_currency(&bytes, number.definition, 0, Some(12345));
    let definition = (number.bytes.len() / PAGE) as u32;
    let catalog_root = u16::from_le_bytes(
        number.bytes[4 * PAGE + 12..4 * PAGE + 14]
            .try_into()
            .unwrap(),
    ) as usize;
    let created = boolean::mixed::append_table(&number.bytes, "Measurements");
    let inserted = boolean::mixed::append(&created, definition);
    let text = boolean::mixed::update_text(&inserted, definition, 0, Some("日本語🦀 middle"));
    let text = boolean::mixed::update_bool(&text, definition, 0, true);
    [
        Case {
            bytes,
            definition: number.definition,
            catalog_root: number.catalog_root,
            table: "Table1",
            column: "Score",
        },
        Case {
            bytes: text,
            definition,
            catalog_root,
            table: "Measurements",
            column: "Label",
        },
    ]
}

fn review(case: &Case) -> RecoverableFieldDropPlan {
    let plan =
        NativeImage::capture_retained(&mut Cursor::new(&case.bytes), SCOPE, &mut budget(), &|| {
            false
        })
        .unwrap()
        .plan_recoverable_retained_non_tail_field_drop(
            case.table,
            case.column,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(plan.intent().is_retained_non_tail());
    assert!(!plan.intent().is_retained() && !plan.intent().is_retained_typed());
    let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(encoded.starts_with(b"TCFD0005"));
    assert!(
        matches!(MutationIntent::decode(&encoded, &mut budget(), &|| false),
        Ok(MutationIntent::RetainedFieldDrop(intent)) if intent.is_retained_non_tail())
    );
    plan
}

fn receipt(receipt: &Receipt) {
    assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnDropped));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
    assert_completion(receipt);
}

fn assert_bytes(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn non_tail_stage_apply_retains_source_until_acceptance_and_publishes_exact_images() {
    for case in cases() {
        let after =
            field_changes::drop_field(&case.bytes, case.definition, case.catalog_root, case.column);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("middle.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &case.bytes).unwrap();
        let staged = open(&path)
            .stage_reviewed_field_drop_with_budgets(
                review(&case),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_staged_read_state(&path, &case.bytes);
        assert!(!bundle.exists());
        let (owner, actual) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        receipt(&actual);
        assert_eq!(actual.kind(), CompletionKind::Applied);
        assert_eq!(actual.outcome(), Some(RecoveryOutcome::Committed));
        owner.release().unwrap();
        assert_bytes(&fs::read(&path).unwrap(), &after);
    }
}

#[test]
fn non_tail_pending_committed_and_settled_recovery_reconstruct_the_same_raw_schema_change() {
    for case in cases() {
        let after =
            field_changes::drop_field(&case.bytes, case.definition, case.catalog_root, case.column);
        for committed in [false, true] {
            for retired in [false, true] {
                assert_retained_schema_recovery(
                    SCOPE,
                    &case.bytes,
                    &after,
                    FieldPlan::Drop(review(&case)),
                    committed,
                    retired,
                    &mut budget(),
                );
            }
        }
    }
}
