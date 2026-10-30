use super::*;
use crate::{
    WriteBlocker,
    table::mutation::insert::tests::{
        incrementing_autonumber::model::{self, State},
        model as original,
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_model::CatalogScope;

const LIMITS: [u64; 3] = [256 * 1024 * 1024, 100_000, 100_000_000];

fn budget() -> ResourceBudget {
    ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2])
}

fn unknown_mode_source() -> Vec<u8> {
    let mut case = model::source(6, State::HighWater);
    let catalog = original::source(6, false, false, Some(("NewValues", "Random")));
    case.before[4 * 4096..5 * 4096].copy_from_slice(&catalog[4 * 4096..5 * 4096]);
    case.before
}

#[test]
fn qualify_generated_binding_sources_and_literal_field_facts_before_helper() {
    for code in [5, 6] {
        let case = model::source(code, State::HighWater);
        let report = original::checked(&case.before, 2, 4);
        let columns = report.schema().columns();
        assert_eq!(columns[0].name(), "Earlier");
        assert_eq!(columns[1].name(), "ID");
        assert_eq!(
            columns[0].scalar_type(),
            Some(tablecraft_schema::ScalarType::Int32)
        );
        assert_eq!(
            &case.before[10 * 4096 + 20..10 * 4096 + 24],
            &260_i32.to_le_bytes()
        );
        let primary = report
            .indexes()
            .iter()
            .find(|index| {
                index
                    .index()
                    .constraints()
                    .is_some_and(|constraints| constraints.primary_key())
            })
            .unwrap();
        assert_eq!(primary.index().columns()[0].number(), 1);
    }
    let bytes = unknown_mode_source();
    original::checked(&bytes, 2, 4);
    let properties = crate::inspect_table_properties(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "Score")
        .unwrap();
    assert_eq!(group.get("NewValues").unwrap().text().unwrap(), "Random");
}

#[test]
fn binding_reports_the_actual_nonfirst_generated_field_and_captured_last() {
    for code in [5, 6] {
        let case = model::source(code, State::HighWater);
        let image = original::capture(&case.before);
        let binding = image
            .prepare_ace16_incrementing_insert_binding("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(binding.generated_position(), 1);
        assert_eq!(binding.generated_name(), "ID");
        assert_eq!(binding.policy().expected_last(), 260);
        assert_eq!(image.as_bytes(), case.before);
    }
    assert!(matches!(
        original::capture(&unknown_mode_source()).prepare_ace16_incrementing_insert_binding(
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Properties))
    ));
}

#[test]
fn name_storage_is_paid_without_changing_the_old_policy_allocation_contract() {
    let case = model::source(6, State::HighWater);
    let image = original::capture(&case.before);
    let mut old = budget();
    let policy = image
        .prepare_ace16_incrementing_insert_policy("Items", &mut old, &|| false)
        .unwrap();
    let mut bound = budget();
    let binding = image
        .prepare_ace16_incrementing_insert_binding("Items", &mut bound, &|| false)
        .unwrap();
    assert_eq!(binding.policy().expected_last(), policy.expected_last());
    assert_eq!(
        old.remaining(Resource::Bytes) - bound.remaining(Resource::Bytes),
        2
    );
    let old_used = LIMITS[0] - old.remaining(Resource::Bytes);
    let mut short = ResourceBudget::new(old_used + 1, LIMITS[1], LIMITS[2]);
    assert!(matches!(
        image.prepare_ace16_incrementing_insert_binding("Items", &mut short, &|| false),
        Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == Resource::Bytes && error.remaining == 1
    ));
}

#[test]
fn cancellation_at_the_actual_final_binding_poll_returns_no_metadata() {
    let case = model::source(6, State::HighWater);
    let image = original::capture(&case.before);
    let polls = Cell::new(0);
    image
        .prepare_ace16_incrementing_insert_binding("Items", &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let terminal = polls.get();
    polls.set(0);
    assert!(matches!(
        image.prepare_ace16_incrementing_insert_binding("Items", &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), terminal);
    assert_eq!(image.as_bytes(), case.before);
}
