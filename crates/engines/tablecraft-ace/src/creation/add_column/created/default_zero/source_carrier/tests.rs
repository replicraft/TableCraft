//! Metadata selects one source purpose; no classifier result grants admission.
use crate::NativeImage;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(773),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn absent_inline_and_external_forwarded_carriers_select_without_mutation() {
    for (case, table) in [
        (raw::Source::IdOnly, "Table1"),
        (raw::Source::GuiRequiredZero, "Table1"),
        (raw::Source::DefaultOnly, "Items"),
    ] {
        let before = raw::source(case);
        let external = raw::repeated::source(case);
        assert!(
            !image(&before)
                .number_column_uses_external_properties(table, &mut budget(), &|| false)
                .unwrap()
        );
        assert!(
            image(&external)
                .number_column_uses_external_properties(table, &mut budget(), &|| false)
                .unwrap()
        );
    }
}

#[test]
fn carrier_observation_keeps_cancellation_and_budget_refusal() {
    let before = raw::repeated::source(raw::Source::IdOnly);
    assert!(
        image(&before)
            .number_column_uses_external_properties("Table1", &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image(&before)
            .number_column_uses_external_properties(
                "Table1",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn older_header_profiles_refuse_as_source_before_user_schema_inspection() {
    use crate::{CreatedAddColumnError, CreatedDefaultZeroAddColumnError};
    use tablecraft_testkit::fixtures::access::{image as ace12, mutation_fixture};
    for (before, table) in [
        (mutation_fixture::database(), "Items"),
        (ace12(false, false), "Table"),
    ] {
        assert!(matches!(
            image(&before).number_column_uses_external_properties(table, &mut budget(), &|| false),
            Err(CreatedDefaultZeroAddColumnError::Created(
                CreatedAddColumnError::Source
            ))
        ));
    }
}
