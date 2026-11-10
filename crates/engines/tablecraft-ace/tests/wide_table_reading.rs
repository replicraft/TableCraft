//! Independently authored wide metadata is readable before creator admission.
use std::io::Cursor;
use tablecraft_ace::{
    NativeImage, PayloadPresence, TableExtraCoverage, inspect_table_schema, read_table,
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
    assert_eq!(schema.columns.len(), 255);
    for (index, column) in schema.columns.iter().enumerate() {
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

    let image = NativeImage::capture(
        &mut Cursor::new(&source.after),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let properties = image
        .inspect_table_extra(SCOPE.object_id(7), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(properties.presence(), PayloadPresence::Present);
    assert_eq!(properties.raw(), Some(source.property_body.as_slice()));
    assert_eq!(properties.coverage(), TableExtraCoverage::WideMr2);
    assert_eq!(properties.names(), ["Required", "DefaultValue"]);
    assert_eq!(properties.groups().len(), 254);
    for group in properties.groups() {
        let fields = group
            .properties()
            .expect("complete authored field properties");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name(), "Required");
        assert_eq!(fields[0].raw(), [0]);
        assert_eq!(fields[1].name(), "DefaultValue");
        assert_eq!(fields[1].text(), Some("0"));
    }
}

#[test]
fn cyclic_or_non_definition_continuations_do_not_yield_a_partial_schema() {
    for next in [8_u32, 10] {
        let mut source = raw::source();
        source.after[8 * 4096 + 4..8 * 4096 + 8].copy_from_slice(&next.to_le_bytes());
        assert!(
            inspect_table_schema(
                &mut Cursor::new(&source.after),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}
