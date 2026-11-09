//! Unequal names move only the index-tail boundary, not stored row geometry.
use super::same_layout;
use crate::inspect_table_schema;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

#[test]
fn exact_name_delta_preserves_layout_but_wrong_boundary_or_row_geometry_refuses() {
    let [before, after] = fitting::pair(SourceCase::NumberFirst, "TaxAmount");
    let scope = CatalogScope::new(987);
    let mut budget = ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000);
    let original = inspect_table_schema(
        &mut Cursor::new(&before),
        scope,
        "Table1",
        &mut budget,
        &|| false,
    )
    .unwrap();
    let mut current = inspect_table_schema(
        &mut Cursor::new(&after),
        scope,
        "Table1",
        &mut budget,
        &|| false,
    )
    .unwrap();

    assert!(same_layout(&original, &current, 8));
    assert!(!same_layout(&original, &current, 0));
    assert!(!same_layout(&original, &current, 6));
    current.layout.fixed_end += 1;
    assert!(!same_layout(&original, &current, 8));
}
