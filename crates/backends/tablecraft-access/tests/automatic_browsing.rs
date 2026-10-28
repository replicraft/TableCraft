#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use fixture as base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
mod attachment_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/page_reads.rs"]
mod page_reads;
use std::io::Cursor;
use tablecraft_access::{
    BrowsePage, CollectionPageRequest, NativeBrowser, StoredCollectionItem,
    StoredCollectionPageValue, TablePageRequest,
};
use tablecraft_ace::{InspectError, TableSnapshotLimits};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

fn request(maximum_rows: u32) -> CollectionPageRequest {
    CollectionPageRequest::new(
        TablePageRequest::new(0, 1).unwrap(),
        TableSnapshotLimits::new(maximum_rows, 1).unwrap(),
    )
}

#[test]
fn ordinary_auto_browse_retains_page_limits_instead_of_complete_snapshot_limits() {
    let bytes = fixture::database();
    let mut browser =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    let BrowsePage::Scalar(page) = browser.browse_page("Items", request(1)).unwrap() else {
        panic!("Ordinary columns selected collection retention");
    };
    assert_eq!(page.rows(), &[vec![Value::Int32(1), Value::Int32(10)]]);
    assert!(!page.reached_end());
}

#[test]
fn automatic_cached_results_remain_stored_and_never_evaluate_the_formula() {
    let bytes = calculated_fixture::database_with_expression(&[Some(77)], "1/0");
    let original = bytes.clone();
    let mut browser =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    let BrowsePage::Collections(page) = browser.browse_page("Items", request(1)).unwrap() else {
        panic!("Cached calculation lost its stored marker metadata");
    };
    assert!(page.features().is_some());
    assert_eq!(page.row_count(), 1);
    assert!(matches!(
        page.cell(0, 1),
        Some(StoredCollectionPageValue::Scalar(Value::Int32(77)))
    ));
    assert_eq!(bytes, original);
}

#[test]
fn automatic_attachments_retain_encoded_values_and_complete_collection_ceilings() {
    let bytes = attachment_fixture::database();
    let original = bytes.clone();
    let mut browser =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    let BrowsePage::Collections(page) = browser.browse_page("Items", request(3)).unwrap() else {
        panic!("Attachment collection became a scalar");
    };
    let Some(StoredCollectionPageValue::Collection(cell)) = page.cell(0, 1) else {
        panic!("Missing explicit attachment collection");
    };
    let StoredCollectionItem::Attachment(attachment) =
        cell.items().unwrap().next().unwrap().unwrap()
    else {
        panic!("Attachment item changed kind");
    };
    assert_eq!(
        attachment.encoded_file_data(),
        Some(attachment_fixture::PAYLOAD)
    );
    let mut limited =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    assert!(limited.browse_page("Items", request(2)).is_err());
    assert_eq!(bytes, original);
}

#[test]
fn automatic_selection_preserves_unsupported_and_corrupt_failures_without_retry() {
    let mut unsupported = fixture::database();
    unsupported[10 * 4096 + 63] = 0xff;
    let mut source = page_reads::PageReads::new(unsupported);
    {
        let mut browser =
            NativeBrowser::new(&mut source, CatalogScope::new(1), budget(), &|| false);
        assert!(matches!(
            browser.browse_page("Items", request(3)),
            Err(InspectError::Unsupported)
        ));
    }
    assert_eq!(source.count(12), 0);
    let mut corrupt = fixture::database();
    corrupt[12 * 4096] = 0;
    let mut source = page_reads::PageReads::new(corrupt);
    {
        let mut browser =
            NativeBrowser::new(&mut source, CatalogScope::new(1), budget(), &|| false);
        assert!(matches!(
            browser.browse_page("Items", request(3)),
            Err(InspectError::Malformed)
        ));
    }
    assert_eq!(source.count(12), 1);
    let bytes = fixture::database();
    let mut cancelled =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            true
        });
    assert!(matches!(
        cancelled.browse_page("Items", request(3)),
        Err(InspectError::Cancelled)
    ));
}
