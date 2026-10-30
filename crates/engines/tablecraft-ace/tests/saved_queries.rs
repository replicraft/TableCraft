use tablecraft_testkit::access::mutation_fixture as base;
#[path = "saved_queries/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use fixture::{DEFINITION_PAGE, EMPTY_ID, Fragment, PAGE_BYTES, QUERY_ID, database, database_with};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use support::put32;
use tablecraft_ace::{
    InspectError, QueryAttribute, SavedQueryDefinition, read_saved_query_definition,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8], name: &str) -> Result<SavedQueryDefinition, InspectError> {
    read_saved_query_definition(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        name,
        &mut budget(),
        &|| false,
    )
}
#[test]
fn saved_fragments_bind_signed_native_ids_and_retain_nullable_unknown_fields_without_execution() {
    for code in [5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let definition = read(&bytes, "sAvEd").unwrap();
        assert_eq!(definition.object().name().as_str(), "Saved");
        assert_eq!(definition.object().kind(), ObjectKind::Query);
        assert_eq!(definition.origin().native_id(), QUERY_ID);
        assert_eq!(definition.origin().flags(), 0x80000030);
        assert_eq!(definition.rows().len(), 2);
        let row = &definition.rows()[0];
        assert_eq!(row.native_object_id(), QUERY_ID);
        assert_eq!(row.attribute(), Some(QueryAttribute::Unknown(200)));
        assert_eq!(row.expression(), Some("NeverRunMe()\t\n\0é🦀"));
        assert_eq!(row.flag(), Some(i16::MIN));
        assert_eq!(row.extra(), Some(i32::MIN));
        assert_eq!(row.name1(), Some("NULL"));
        assert_eq!(row.name2(), Some(""));
        assert_eq!(row.order(), Some([9, 8, 7, 6].as_slice()));
        let row = &definition.rows()[1];
        assert_eq!(row.attribute(), None);
        assert_eq!(row.expression(), None);
        assert_eq!(row.flag(), None);
        assert_eq!(row.extra(), None);
        assert_eq!(row.name1(), None);
        assert_eq!(row.name2(), None);
        assert_eq!(row.order(), Some([].as_slice()));
    }
}

#[test]
fn query_namespace_empty_definitions_and_caller_scopes_do_not_become_table_references() {
    let bytes = database();
    assert_eq!(
        read(&bytes, "missing").err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(
        read(&bytes, "Items").err(),
        Some(InspectError::UnknownObject)
    );
    let empty = read(&bytes, "Empty").unwrap();
    assert!(empty.rows().is_empty());
    assert_eq!(empty.origin().native_id(), EMPTY_ID);
    let first = read(&bytes, "Saved").unwrap();
    let second = read_saved_query_definition(
        &mut Cursor::new(&bytes),
        CatalogScope::new(2),
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(first.object().id(), second.object().id());
    assert_eq!(first.origin().native_id(), second.origin().native_id());
}

#[test]
fn incompatible_system_carriers_and_late_geometry_fail_without_mutating_input() {
    let mut bytes = database();
    bytes[DEFINITION_PAGE * PAGE_BYTES + 63 + 6 * 25] = 9;
    assert_eq!(read(&bytes, "Saved").err(), Some(InspectError::Unsupported));
    let mut bytes = database();
    put32(&mut bytes, DEFINITION_PAGE * PAGE_BYTES + 16, 4);
    assert_eq!(read(&bytes, "Saved").err(), Some(InspectError::Malformed));
}

#[test]
fn saved_query_reads_observe_budgets_cancellation_and_original_bytes() {
    let original = database();
    let mut source = Cursor::new(original.clone());
    let count = Cell::new(0);
    read_saved_query_definition(
        &mut source,
        CatalogScope::new(1),
        "Saved",
        &mut budget(),
        &|| {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [0, count.get() / 2, count.get() - 1] {
        let calls = Cell::new(0);
        assert!(matches!(
            read_saved_query_definition(
                &mut source,
                CatalogScope::new(1),
                "Saved",
                &mut budget(),
                &|| {
                    let at = calls.get();
                    calls.set(at + 1);
                    at >= stop
                }
            ),
            Err(InspectError::Cancelled)
        ));
    }
    for mut limited in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            read_saved_query_definition(
                &mut source,
                CatalogScope::new(1),
                "Saved",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(source.into_inner(), original);
}

proptest! {
    #![proptest_config(ProptestConfig {cases:128,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x5155455259524f57),..Default::default()})]
    #[test]
    fn fragment_metadata_retains_every_generated_carrier_and_nullable_state(
        values in prop::collection::vec((prop::option::of(any::<u8>()),
            prop::option::of(".{0,32}".prop_map(|value|format!("expr:{value}"))),
            prop::option::of(any::<i16>()),prop::option::of(any::<i32>()),
            prop::option::of(prop::collection::vec(any::<u8>(),0..=4))),0..8)
    ) {
        let fragments = values.iter().map(|(attribute, expression, flag, extra, order)| Fragment {
            object: Some(QUERY_ID), attribute: *attribute, expression: expression.as_deref(),
            flag: *flag, extra: *extra, order: order.as_deref(), ..Default::default()
        }).collect::<Vec<_>>();
        let bytes = database_with(&fragments);
        let definition = read(&bytes,"Saved").unwrap();
        let actual = definition.rows().iter().map(|row| (
            row.attribute().map(QueryAttribute::code),row.expression().map(str::to_owned),
            row.flag(),row.extra(),row.order().map(<[u8]>::to_vec)
        )).collect::<Vec<_>>();
        prop_assert_eq!(actual,values);
    }
}
