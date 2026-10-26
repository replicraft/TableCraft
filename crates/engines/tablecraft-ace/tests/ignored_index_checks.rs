#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "ignored_index_checks/boundaries.rs"]
mod boundaries;
#[path = "ignored_index_checks/fixture.rs"]
mod corpus;
#[allow(dead_code)]
#[path = "decimal_composite_indexes/fixture.rs"]
mod decimal_fixture;
#[path = "ignored_index_checks/fixed.rs"]
mod fixed;
#[allow(dead_code)]
#[path = "ignored_null_indexes/fixture.rs"]
mod ignored_page;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
#[path = "ignored_index_checks/payloads.rs"]
mod payloads;
mod support;
#[path = "ignored_index_checks/tuples.rs"]
mod tuples;

use base as fixture;
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, InspectError, TableCheck, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 32_768, 4_000_000)
}

fn check(bytes: &[u8]) -> Result<TableCheck, InspectError> {
    check_table(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
}

fn checked(bytes: &[u8], rows: usize) {
    let report = check(bytes).unwrap();
    assert_eq!(report.rows(), rows);
    assert_eq!(report.schema().declared_rows() as usize, rows);
    assert_eq!(report.indexes().len(), 1);
    assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
}
