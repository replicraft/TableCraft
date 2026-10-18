use std::io::Cursor;
use tablecraft_ace::{InspectError, PropertyGroupKind, inspect_table_properties};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn copy() -> Cursor<Vec<u8>> {
    Cursor::new(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../docs/examples/Access_Example_VBA.accdb"),
        )
        .unwrap(),
    )
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

#[test]
fn real_field_rules_and_opaque_metadata_are_inspected_without_changing_bytes() {
    let mut source = copy();
    let original = source.get_ref().clone();
    let properties = inspect_table_properties(
        &mut source,
        CatalogScope::new(1),
        "roulette_tb",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.name(), "ROULETTE_TB");
    assert!(properties.raw_bytes().starts_with(b"MR2\0"));
    let table = properties.group(PropertyGroupKind::Table, "").unwrap();
    assert_eq!(
        table.get("Description").unwrap().text().unwrap(),
        "Numbers for roulette"
    );
    let column = properties
        .group(PropertyGroupKind::Column, "nbr_value")
        .unwrap();
    assert!(column.get("Required").unwrap().boolean().unwrap());
    assert!(!column.get("AllowZeroLength").unwrap().boolean().unwrap());
    assert!(column.get("DefaultValue").is_none());
    let guid = table.get("GUID").unwrap();
    assert_eq!(guid.raw_value().len(), 16);
    assert_eq!(guid.native_type(), 9);
    assert!(guid.text().is_err());
    assert_eq!(source.get_ref(), &original);
}

#[test]
fn properties_share_catalog_row_work_byte_and_cancellation_limits() {
    assert!(matches!(
        inspect_table_properties(
            &mut copy(),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    for budget in [
        ResourceBudget::new(0, 1000, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1000),
        ResourceBudget::new(16 * 1024 * 1024, 1000, 0),
    ] {
        assert!(matches!(
            inspect_table_properties(
                &mut copy(),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut { budget },
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert!(matches!(
        inspect_table_properties(
            &mut copy(),
            CatalogScope::new(1),
            "missing",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::UnknownTable)
    ));
}
