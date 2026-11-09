//! Legacy proof mapping preserves complete output/order without a second replay.
use super::*;
use crate::CreatedDefaultZeroColumnRequired;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_created_number::{
    self as number, mixed as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(947);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn first_and_repeated_number_proofs_map_to_identical_independent_text_afters() {
    for (case, table) in [
        (number::Source::IdOnly, "Table1"),
        (number::Source::GuiRequiredZero, "Table1"),
        (number::Source::DefaultOnly, "Items"),
    ] {
        let initial = number::source(case);
        let external = number::repeated::source(case);
        for (before, repeated) in [(initial, false), (external, true)] {
            let source = image(&before);
            if repeated {
                source
                    .check_repeated_default_zero_add_column_source(table, &mut budget(), &|| false)
                    .unwrap();
            } else {
                source
                    .check_created_default_zero_add_column_source(table, &mut budget(), &|| false)
                    .unwrap();
            }
            let proved = capture(&source, table, &mut budget(), &|| false).unwrap();
            assert_eq!(proved.external, repeated);
            assert_eq!(proved.checked.schema().name(), table);
            let expected = raw::append_text(&before, "Label", 80, true);
            let candidate = image(&before)
                .prepare_created_add_field_candidate(
                    table,
                    "Label",
                    FieldAdditionTarget::ShortText {
                        max_utf16_units: 80,
                        unicode_compression: true,
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.before_bytes(), before);
            assert_eq!(candidate.after_bytes(), expected);
        }
    }
}

#[test]
fn generated_policy_guard_remains_explicit_for_every_mapped_proof() {
    let generated = CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    generated_policy(&[generated]).unwrap();
    for field in [
        generated.with_required(false),
        generated.with_description("Generated description").unwrap(),
    ] {
        assert!(matches!(generated_policy(&[field]), Err(Error::Source)));
    }
    // Public declaration checks also prevent manufacturing generated defaults
    // or a Text-only empty-value/compression permission on this LONG.
    assert!(generated.with_default_literal(&Value::Int32(0)).is_err());
    assert!(generated.with_allow_zero_length(false).is_err());
    assert!(generated.with_unicode_compression(true).is_err());
    assert!(matches!(generated_policy(&[]), Err(Error::Source)));
}

#[test]
fn corrupt_legacy_history_or_complement_cannot_retry_as_a_typed_source() {
    let before = number::repeated::source(number::Source::IdOnly);
    for offset in [4096 - 1, 9 * 4096 + 4095, 10 * 4096 + 4095] {
        let mut changed = before.clone();
        changed[offset] ^= 1;
        let source = image(&changed);
        assert!(
            source
                .check_repeated_default_zero_add_column_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
        assert!(capture(&source, "Table1", &mut budget(), &|| false).is_err());
    }
    // The first-Number declaration remains the old explicit StoredFalse
    // language; mapping cannot replace it with Required-absent provenance.
    let first = image(&number::source(number::Source::IdOnly))
        .prepare_created_add_default_zero_int32_column_candidate(
            "Table1",
            "Score",
            CreatedDefaultZeroColumnRequired::Absent,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(
        capture(
            &image(first.after_bytes()),
            "Table1",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn mapped_proof_cancellation_and_zero_budget_never_become_source_availability() {
    let source = image(&number::repeated::source(number::Source::IdOnly));
    assert!(matches!(
        capture(&source, "Table1", &mut budget(), &|| true),
        Err(Error::Inspect(crate::InspectError::Cancelled))
    ));
    assert!(matches!(
        capture(
            &source,
            "Table1",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(Error::Inspect(crate::InspectError::Limit(_)))
    ));
}
