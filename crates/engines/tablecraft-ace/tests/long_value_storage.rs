#[path = "long_value_storage/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "long_value_storage/model.rs"]
mod model;
use tablecraft_testkit::access as support;
#[path = "long_value_storage/values.rs"]
mod values;

use std::io::Cursor;
use tablecraft_ace::{InspectError, LongValueCheckStatus, check_table_long_values, read_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

fn check(bytes: &[u8]) -> Result<tablecraft_ace::TableLongValueCheck, InspectError> {
    check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}

fn readable(bytes: &[u8]) {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), 5);
}
