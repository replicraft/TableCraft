#[path = "table_schema/chains.rs"]
mod chains;
#[path = "table_schema/short_values.rs"]
mod short_values;
use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_table_schema};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access as support;
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
fn real_table_schema_maps_supported_columns_and_keeps_unsupported_fields_visible() {
    let mut bytes = copy();
    let original = bytes.get_ref().clone();
    let schema = inspect_table_schema(
        &mut bytes,
        CatalogScope::new(1),
        "roulette_tb",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.name(), "ROULETTE_TB");
    assert_eq!(schema.declared_rows(), 38);
    assert_eq!(schema.definition_page(), 726);
    let cols = schema.scalar_columns().unwrap();
    assert_eq!(
        cols.iter()
            .map(|c| (c.name(), c.data_type()))
            .collect::<Vec<_>>(),
        vec![
            ("ORDR_NBR", ScalarType::Int32),
            ("NBR_VALUE", ScalarType::Text),
            ("COLOR_ID", ScalarType::Int32)
        ]
    );
    let schema = inspect_table_schema(
        &mut bytes,
        CatalogScope::new(1),
        "CMD_LINE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns()[1].name(), "DESCRIPTION");
    assert_eq!(schema.columns()[1].native_type(), 12);
    assert!(schema.scalar_columns().is_ok());
    let schema = inspect_table_schema(
        &mut bytes,
        CatalogScope::new(1),
        "MSysResources",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(schema.columns()[0].scalar_type().is_none());
    assert_eq!(bytes.into_inner(), original);
}
#[test]
fn schema_lookup_and_definition_failures_are_explicit() {
    assert_eq!(
        inspect_table_schema(
            &mut copy(),
            CatalogScope::new(1),
            "missing-private-name",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::UnknownTable)
    );
    let mut bytes = copy();
    bytes.get_mut()[726 * 4096 + 4] = 1;
    assert_eq!(
        inspect_table_schema(
            &mut bytes,
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Malformed)
    );
    assert!(
        inspect_table_schema(
            &mut copy(),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| true
        )
        .is_err()
    );
    assert!(
        inspect_table_schema(
            &mut copy(),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut ResourceBudget::new(66, 1000, 1000),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn linked_table_metadata_never_opens_a_native_definition_or_provider() {
    for kind in [4_u16, 6] {
        let mut bytes = support::image(false, false);
        let at = 4 * 4096;
        let start = (u16::from_le_bytes([bytes[at + 14], bytes[at + 15]]) & 0x1fff) as usize;
        support::put16(&mut bytes, at + start + 10, kind);
        assert_eq!(
            inspect_table_schema(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Table",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}
