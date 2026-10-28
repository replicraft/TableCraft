#[path = "mutation/fixture.rs"]
mod base;
#[path = "complex_schema/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_complex_schema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn inspect(bytes: Vec<u8>) -> Result<tablecraft_ace::ComplexTableSchema, InspectError> {
    inspect_complex_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}
#[test]
fn complex_references_resolve_to_checked_child_geometry_without_reading_parent_values() {
    let schema = inspect(fixture::database()).unwrap();
    assert_eq!(schema.columns().len(), 1);
    let column = &schema.columns()[0];
    assert_eq!((column.number(), column.type_id()), (1, 7));
    assert_eq!(column.type_table().name(), "ValueType");
    assert_eq!(column.flat_table().name(), "Children");
    assert_eq!(
        (column.foreign_key_number(), column.identity_number()),
        (0, 2)
    );
    assert!(schema.owner().scalar_columns().is_err());
}
#[test]
fn missing_duplicate_wrong_owner_and_self_referential_mappings_fail_atomically() {
    for rows in [
        vec![],
        vec![fixture::mapping(7, 10, 20, 17); 2],
        vec![fixture::mapping(7, 99, 20, 17)],
        vec![fixture::mapping(8, 10, 20, 17)],
        vec![fixture::mapping(7, 10, 10, 17)],
        vec![fixture::mapping(7, 10, 20, 20)],
        vec![fixture::mapping(7, 10, 999, 17)],
    ] {
        assert!(inspect(fixture::with_mappings(rows)).is_err());
    }
}
#[test]
fn wrong_child_keys_or_value_geometry_never_qualify() {
    for (offset, value) in [
        (20 * 4096 + 63 + 16, 0),
        (20 * 4096 + 63 + 50 + 15, 3),
        (20 * 4096 + 63 + 25, 2),
        (10 * 4096 + 63 + 25 + 11, 0),
    ] {
        let mut bytes = fixture::database();
        bytes[offset] = value;
        assert!(inspect(bytes).is_err());
    }
}
#[test]
fn scalar_tables_need_no_complex_system_catalog_and_cancellation_is_shared() {
    assert!(
        inspect(base::with_rule("Caption", "scalar metadata"))
            .unwrap()
            .columns()
            .is_empty()
    );
    let bytes = fixture::database();
    assert_eq!(
        inspect_complex_schema(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(
        inspect_complex_schema(
            &mut Cursor::new(fixture::database()),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .is_err()
    );
}
