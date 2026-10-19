#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use std::io::Cursor;
use tablecraft_access::{NativeBrowser, TablePageRequest};
use tablecraft_ace::InspectError;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn library_inventory_and_pages_are_usable_without_cli_or_query_evaluation() {
    let mut bytes = fixture::database();
    const PAGE_BYTES: usize = 4096;
    const CATALOG_DEFINITION_PAGE: usize = 2;
    const RECORD_COUNT_OFFSET: usize = 16;
    const CATALOG_DATA_PAGE: usize = 4;
    support::put32(
        &mut bytes,
        CATALOG_DEFINITION_PAGE * PAGE_BYTES + RECORD_COUNT_OFFSET,
        4,
    );
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        support::record(42, 0, 5, 0, b"\xff\xfeSaved query"),
    ];
    bytes[CATALOG_DATA_PAGE * PAGE_BYTES..(CATALOG_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &support::data_page(
            2,
            &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
        ),
    );
    let original = bytes.clone();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    );
    let inventory = browser.tables().unwrap();
    assert_eq!(
        inventory
            .tables()
            .map(|t| t.name().as_str())
            .collect::<Vec<_>>(),
        ["MSysObjects", "MSysRelationships", "Items"]
    );
    let page = browser
        .page("Items", TablePageRequest::new(1, 1).unwrap())
        .unwrap();
    assert_eq!(page.rows(), &[vec![Value::Int32(2), Value::Int32(20)]]);
    let end = browser
        .page("Items", TablePageRequest::new(3, 1).unwrap())
        .unwrap();
    assert!(end.rows().is_empty());
    assert!(end.reached_end());
    assert_eq!(bytes, original);
}
#[test]
fn operations_share_budget_and_cancellation_instead_of_resetting_controls() {
    let bytes = fixture::database();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 4, 1_000_000),
        &|| false,
    );
    browser.tables().unwrap();
    assert!(matches!(
        browser.page("Items", TablePageRequest::new(0, 1).unwrap()),
        Err(InspectError::Limit(_))
    ));
    let mut cancelled = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| true,
    );
    assert!(matches!(cancelled.tables(), Err(InspectError::Cancelled)));
    assert!(matches!(
        cancelled.page("Items", TablePageRequest::new(0, 1).unwrap()),
        Err(InspectError::Cancelled)
    ));
}
