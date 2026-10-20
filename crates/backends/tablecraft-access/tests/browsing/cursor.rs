use super::*;
use std::cell::Cell;

#[test]
fn cursor_borrows_original_controls_and_owned_batches_outlive_it() {
    let bytes = fixture::database();
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &callback,
    );
    let batch;
    {
        let mut cursor = browser.cursor("Items").unwrap();
        assert_eq!(cursor.columns().len(), 2);
        assert_eq!(cursor.schema().declared_rows(), 3);
        batch = cursor.next_page(1).unwrap();
        assert_eq!(cursor.position(), 1);
        cancelled.set(true);
        assert!(matches!(cursor.next_page(1), Err(InspectError::Cancelled)));
        assert!(cursor.is_failed());
        assert!(matches!(
            cursor.next_page(1),
            Err(InspectError::CursorFailed)
        ));
    }
    assert_eq!(batch.rows(), &[vec![Value::Int32(1), Value::Int32(10)]]);
    cancelled.set(false);
    assert!(browser.describe("Items").is_ok());
}

#[test]
fn batches_do_not_refill_the_browser_row_budget() {
    let bytes = fixture::database();
    // Opening spends three catalog rows; exactly one decoded user row remains.
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 4, 1_000_000),
        &|| false,
    );
    let mut cursor = browser.cursor("Items").unwrap();
    cursor.next_page(1).unwrap();
    assert!(matches!(cursor.next_page(1), Err(InspectError::Limit(_))));
    assert_eq!(cursor.position(), 1);
    assert!(cursor.is_failed());
}
