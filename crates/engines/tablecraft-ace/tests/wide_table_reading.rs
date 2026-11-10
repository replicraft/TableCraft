//! Independently authored wide metadata is readable before creator admission.
use std::io::Cursor;
use tablecraft_ace::{
    inspect_table_properties, inspect_table_schema, read_table, PropertyGroupKind,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

const SCOPE: CatalogScope = CatalogScope::new(255);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

#[test]
fn all_255_columns_and_external_field_properties_are_read_from_independent_bytes() {
    let source = raw::source();
    let schema = inspect_table_schema(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 255);
    for (index, column) in schema.columns().iter().enumerate() {
        assert_eq!(column.name(), raw::field_name(index));
        assert_eq!(column.number(), index);
        assert_eq!(column.native_type(), 4);
        assert_eq!(column.storage_length(), 4);
        assert!(column.fixed());
    }
    let rows = read_table(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(rows.rows().is_empty());

    let properties = inspect_table_properties(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.object_id(), SCOPE.object_id(7));
    assert_eq!(properties.raw_bytes(), source.property_body);
    assert_eq!(properties.groups().len(), 254);
    for (index, group) in properties.groups().iter().enumerate() {
        assert_eq!(group.kind(), PropertyGroupKind::Column);
        assert_eq!(group.name(), raw::field_name(index + 1));
        let fields = group.properties();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name(), "Required");
        assert!(!fields[0].boolean().unwrap());
        assert_eq!(fields[1].name(), "DefaultValue");
        assert_eq!(fields[1].text().unwrap(), "0");
    }
}

#[test]
fn cyclic_or_non_definition_continuations_do_not_yield_a_partial_schema() {
    for next in [8_u32, 10] {
        let mut source = raw::source();
        source.after[8 * 4096 + 4..8 * 4096 + 8].copy_from_slice(&next.to_le_bytes());
        assert!(inspect_table_schema(
            &mut Cursor::new(&source.after),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .is_err());
    }
}

#[test]
fn cyclic_external_property_storage_returns_no_partial_property_map() {
    let mut source = raw::source();
    // The first literal LVAL record starts at byte20 of page12. Repeating that
    // reference must not publish its valid prefix as a complete property stream.
    source.after[12 * 4096 + 20..12 * 4096 + 24].copy_from_slice(&(12_u32 << 8).to_le_bytes());
    assert!(inspect_table_properties(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .is_err());
}

#[test]
fn full_length_text_metadata_crosses_64_property_pages_without_losing_fields() {
    let source = raw::property_windows::source();
    assert_eq!(source.after.len(), 95 * 4096);
    assert_eq!(source.property_pages.len(), 75);
    let schema = inspect_table_schema(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 255);
    assert_eq!(schema.columns()[0].name(), "ID");
    for (index, column) in schema.columns().iter().enumerate().skip(1) {
        assert_eq!(column.name(), raw::property_windows::field_name(index));
        assert_eq!(column.name().encode_utf16().count(), 64);
        assert_eq!(column.number(), index);
        assert_eq!(column.native_type(), 10);
        assert_eq!(column.storage_length(), 510);
        assert!(!column.fixed());
    }
    let properties = inspect_table_properties(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.raw_bytes(), source.property_body);
    assert_eq!(properties.groups().len(), 254);
    for (index, group) in properties.groups().iter().enumerate() {
        assert_eq!(group.kind(), PropertyGroupKind::Column);
        assert_eq!(group.name(), raw::property_windows::field_name(index + 1));
        let fields = group.properties();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0].name(), "Required");
        assert!(!fields[0].boolean().unwrap());
        assert_eq!(fields[1].name(), "AllowZeroLength");
        assert!(fields[1].boolean().unwrap());
        assert_eq!(fields[2].name(), "DefaultValue");
        assert_eq!(
            fields[2].text().unwrap(),
            raw::property_windows::default_expression()
        );
        assert_eq!(fields[3].name(), "Description");
        assert_eq!(fields[3].native_type(), 10);
        assert_eq!(
            fields[3].text().unwrap(),
            raw::property_windows::description()
        );
    }
    let rows = read_table(
        &mut Cursor::new(&source.after),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(rows.rows().is_empty());
    // No oversized default-filled row is constructed by this metadata witness.
}
