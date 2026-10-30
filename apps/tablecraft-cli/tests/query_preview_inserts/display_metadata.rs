//! Reuse the actual SQL INSERT source and complete expected-image owner.
#[path = "display_metadata/api.rs"]
mod api;

use super::*;

const PAGE_BYTES: usize = 4096;

fn display_case() -> (Vec<u8>, Vec<u8>) {
    let mut before = source(6, true);
    let property_source = fixture::with_rule("Format", "000000");
    before[4 * PAGE_BYTES..5 * PAGE_BYTES]
        .copy_from_slice(&property_source[4 * PAGE_BYTES..5 * PAGE_BYTES]);
    let wanted = expected(&before, true);
    (before, wanted)
}

#[test]
fn qualify_cli_generated_display_source_and_complete_expected_before_action() {
    let (before, wanted) = display_case();
    checked(&before, 3);
    checked(&wanted, 4);
    let properties = tablecraft_ace::inspect_table_properties(
        &mut Cursor::new(&before),
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
    let format = group.get("Format").unwrap();
    assert_eq!(format.native_type(), 10);
    assert_eq!(format.flags(), 0);
    assert_eq!(format.text().unwrap(), "000000");
    assert_eq!(
        &wanted[4 * PAGE_BYTES..5 * PAGE_BYTES],
        &before[4 * PAGE_BYTES..5 * PAGE_BYTES]
    );
    let table = tablecraft_ace::read_table(
        &mut Cursor::new(&wanted),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[3], [Value::Int32(4), Value::Int32(31)]);
}
