use super::{fixture as base, support};
use std::io::Cursor;
use tablecraft_access::{CollectionPageRequest, NativeBrowser, TablePageRequest};
use tablecraft_ace::{
    InspectError, TableSnapshotLimits, read_stored_collection_table_page_with_request,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

#[path = "../../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;

#[test]
fn multivalue_pages_keep_explicit_collection_cells_at_the_shared_browsing_boundary() {
    use tablecraft_access::MultivaluePageValue;
    let bytes = links_fixture::populated();
    let mut browser = NativeBrowser::new(
        Cursor::new(bytes),
        CatalogScope::new(1),
        ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    );
    let page = browser
        .multivalue_page("Items", TablePageRequest::new(1, 1).unwrap(), false)
        .unwrap();
    assert_eq!(page.row_count(), 1);
    assert_eq!(page.schema().name(), "Items");
    match page.cell(0, 1).unwrap() {
        MultivaluePageValue::Collection(value) => {
            assert_eq!(value.key(), Some(20));
            assert_eq!(
                value.values().unwrap().collect::<Vec<_>>(),
                [&Value::Int32(200)]
            );
        }
        _ => panic!("Collection became a scalar"),
    }
}

#[test]
fn explicit_collection_page_limits_share_one_original_browser_budget() {
    let bytes = links_fixture::populated();
    let request = CollectionPageRequest::new(
        TablePageRequest::new(0, 1).unwrap(),
        TableSnapshotLimits::new(3, 1).unwrap(),
    );
    let mut baseline = ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000);
    read_stored_collection_table_page_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        request,
        &mut baseline,
        &|| false,
    )
    .unwrap();
    let consumed = 32 * 1024 * 1024 - baseline.remaining(Resource::Bytes);
    let mut browser = NativeBrowser::new(
        Cursor::new(bytes),
        CatalogScope::new(1),
        ResourceBudget::new(consumed, 16_384, 1_000_000),
        &|| false,
    );
    assert_eq!(
        browser
            .collection_page_with_request("Items", request)
            .unwrap()
            .row_count(),
        1
    );
    assert!(matches!(
        browser.collection_page_with_request("Items", request),
        Err(InspectError::Limit(_))
    ));
}
