//! Exact Boolean SQL carriers reach the retained bit writer and reviewed save.
use super::{budget, open};
use crate::editing::{
    offline::test_support::{assert_completion, assert_staged_read_state},
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile, RecoverableNativeEditPlan};
use tablecraft_model::CatalogScope;
use tablecraft_query::UpdatePlan;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{boolean as raw, fields};

fn review(before: &[u8], statement: &str) -> UpdatePlan<RecoverableNativeEditPlan> {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(1751),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query(statement, ParseLimits::default()).unwrap();
    sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
        .unwrap()
}

#[test]
fn reviewed_true_and_false_save_only_the_known_boolean_value_bit() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "BooleanValues");
    let mut before = raw::append(&empty, definition, raw::Input::Omitted);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("boolean.accdb");
    fs::write(&path, &before).unwrap();
    for (step, statement, value) in [
        (0, "UPDATE BooleanValues SET Enabled=True WHERE ID=1", true),
        (
            1,
            "UPDATE BooleanValues SET Enabled=False WHERE ID=1",
            false,
        ),
    ] {
        let after = raw::update(&before, definition, 0, value);
        let reviewed = review(&before, statement);
        assert_eq!(
            reviewed.backend().intent().profile(),
            NativeUpdateProfile::Ace16RetainedBooleanRows
        );
        assert!(
            reviewed
                .backend()
                .intent()
                .encode(&mut budget(), &|| false)
                .unwrap()
                .starts_with(b"TCNI0020")
        );
        assert_eq!(reviewed.backend().after_bytes(), after);
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_staged_read_state(&path, &before);
        let bundle = directory.path().join(format!("journal-{step}"));
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        before = after;
    }
}

#[test]
fn retained_mixed_table_keeps_boolean_currency_long_and_text_request_families() {
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::boolean::mixed;

    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = mixed::append_table(&seed, "MixedValues");
    let mut before = mixed::append(&empty, definition);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mixed-boolean.accdb");
    fs::write(&path, &before).unwrap();
    for step in 0..5 {
        let (statement, profile, after) = match step {
            0 => (
                "UPDATE MixedValues SET Enabled=True WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedBooleanRows,
                mixed::update_bool(&before, definition, 0, true),
            ),
            1 => (
                "UPDATE MixedValues SET Amount=CCur(1.2345) WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedCurrencyRows,
                mixed::update_currency(&before, definition, 0, Some(12_345)),
            ),
            2 => (
                "UPDATE MixedValues SET Score=41 WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedRows,
                mixed::update_number(&before, definition, 0, Some(41)),
            ),
            3 => (
                "UPDATE MixedValues SET Label='Mixed🦀' WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedRows,
                mixed::update_text(&before, definition, 0, Some("Mixed🦀")),
            ),
            _ => (
                "UPDATE MixedValues SET Enabled=False WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedBooleanRows,
                mixed::update_bool(&before, definition, 0, false),
            ),
        };
        let reviewed = review(&before, statement);
        assert_eq!(reviewed.backend().intent().profile(), profile);
        assert_eq!(reviewed.backend().after_bytes(), after);
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        let (owner, receipt) = staged
            .apply(
                &directory.path().join(format!("journal-{step}")),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
        before = after;
    }
}
