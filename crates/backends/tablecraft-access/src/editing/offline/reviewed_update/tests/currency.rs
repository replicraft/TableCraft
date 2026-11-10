//! Currency target metadata selects TCNI19 while retained LONG/Text keeps17.
use super::{budget, open};
use crate::editing::{
    offline::{
        CompletionKind,
        test_support::{assert_completion, assert_staged_read_state},
    },
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile, RecoverableNativeEditPlan};
use tablecraft_model::CatalogScope;
use tablecraft_query::UpdatePlan;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{currency as raw, fields};

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "MoneyTable");
    let first = raw::append(&empty, definition, raw::Input::Omitted);
    (
        raw::append(&first, definition, raw::Input::Value(12_345)),
        definition,
    )
}

fn review(before: &[u8], statement: &str) -> UpdatePlan<RecoverableNativeEditPlan> {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(1720),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query(statement, ParseLimits::default()).unwrap();
    sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
        .unwrap()
}

#[test]
fn reviewed_currency_sql_saves_exact_scale_then_null_and_refill() {
    let (mut before, definition) = source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("currency.accdb");
    fs::write(&path, &before).unwrap();
    for (step, statement, value) in [
        (
            0,
            "UPDATE MoneyTable SET Amount=-2.0001 WHERE ID=2",
            Some(-20_001),
        ),
        (1, "UPDATE MoneyTable SET Amount=Null WHERE ID=2", None),
        (2, "UPDATE MoneyTable SET Amount=0.0001 WHERE ID=2", Some(1)),
    ] {
        let expected = raw::update(&before, definition, 1, value);
        let reviewed = review(&before, statement);
        assert_eq!(
            reviewed.backend().intent().profile(),
            NativeUpdateProfile::Ace16RetainedCurrencyRows
        );
        assert!(
            reviewed
                .backend()
                .intent()
                .encode(&mut budget(), &|| false)
                .unwrap()
                .starts_with(b"TCNI0019")
        );
        assert_eq!(reviewed.backend().after_bytes(), expected);
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_staged_read_state(&path, &before);
        let bundle = directory.path().join(format!("journal-{step}"));
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        before = expected;
    }
}

#[test]
fn one_retained_table_keeps_currency_number_and_text_action_profiles_independent() {
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::currency::mixed;

    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = mixed::append_table(&seed, "MixedValues");
    let mut before = mixed::append(&empty, definition);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mixed.accdb");
    fs::write(&path, &before).unwrap();
    for step in 0..3 {
        let (statement, profile, expected) = match step {
            0 => (
                "UPDATE MixedValues SET Amount=1.2345 WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedCurrencyRows,
                mixed::update_currency(&before, definition, 0, Some(12_345)),
            ),
            1 => (
                "UPDATE MixedValues SET Score=41 WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedRows,
                mixed::update_number(&before, definition, 0, Some(41)),
            ),
            _ => (
                "UPDATE MixedValues SET Label='Mixed🦀' WHERE ID=1",
                NativeUpdateProfile::Ace16RetainedRows,
                mixed::update_text(&before, definition, 0, Some("Mixed🦀")),
            ),
        };
        let reviewed = review(&before, statement);
        assert_eq!(reviewed.backend().intent().profile(), profile);
        assert_eq!(reviewed.backend().after_bytes(), expected);
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
        assert_eq!(fs::read(&path).unwrap(), expected);
        before = expected;
    }
}
