//! Explicit scalar pages never enter the complete collection payload path.
use super::*;

#[test]
fn projected_attachment_parent_page_does_not_open_child_records_or_payloads() {
    let bytes = attachment_fixture::database();
    let original = bytes.clone();
    let mut input = page_reads::PageReads::new(bytes.clone());
    {
        let mut browser = NativeBrowser::new(&mut input, CatalogScope::new(1), budget(), &|| false);
        let page = browser
            .projected_page("Items", &["Id"], TablePageRequest::new(1, 1).unwrap())
            .unwrap();
        assert_eq!(page.rows(), [vec![Value::Int32(2)]]);
        assert_eq!(page.schema().columns().len(), 2);
        assert_eq!(page.columns().len(), 1);
        assert!(!page.reached_end());
    }
    assert_eq!(input.count(attachment_fixture::CHILD_PAGE as u64), 0);
    assert_eq!(input.count(attachment_fixture::FLAT_PAGE as u64), 0);
    assert_eq!(input.count(23), 0);
    let mut browser =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    assert!(browser.browse_page("Items", request(2)).is_err());
    assert!(matches!(
        browser.browse_page("Items", request(3)).unwrap(),
        BrowsePage::Collections(_)
    ));
    assert_eq!(bytes, original);
}

#[test]
fn projected_browser_keeps_original_budgets_and_does_not_retry_record_failures() {
    let bytes = fixture::database();
    // Three catalog rows plus exactly one selected row, as in cursor controls.
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 4, 1_000_000),
        &|| false,
    );
    let page = browser
        .projected_page("Items", &["Id"], TablePageRequest::new(0, 1).unwrap())
        .unwrap();
    assert_eq!(page.rows(), [vec![Value::Int32(1)]]);
    assert!(matches!(
        browser.projected_page("Items", &["Id"], TablePageRequest::new(1, 1).unwrap()),
        Err(InspectError::Limit(_))
    ));

    let mut bytes = bytes;
    let start = usize::from(
        u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) & 0x1fff,
    );
    support::put16(&mut bytes, 12 * 4096 + start, u16::MAX);
    let mut input = page_reads::PageReads::new(bytes);
    {
        let mut browser = NativeBrowser::new(&mut input, CatalogScope::new(1), budget(), &|| false);
        assert!(
            browser
                .projected_page("Items", &["Id"], TablePageRequest::new(0, 1).unwrap())
                .is_err()
        );
    }
    assert_eq!(input.count(12), 1);
}
