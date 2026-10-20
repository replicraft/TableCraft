//! Resumable browsing with the host's existing cumulative controls.
use super::*;
use tablecraft_ace::{TableCursor, TableSchema};
use tablecraft_schema::Column;

pub use tablecraft_ace::TableBatch;

/// A source-bound scalar cursor borrowing its browser's cumulative controls.
/// Drop it before starting another operation on the browser. A failed fetch is
/// terminal; previously returned batches remain valid owned values.
pub struct BrowserCursor<'a, R> {
    cursor: TableCursor<'a, R>,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
}
impl<R: Read + Seek> BrowserCursor<'_, R> {
    /// Captured native schema, with an initially unverified row declaration.
    pub fn schema(&self) -> &TableSchema {
        self.cursor.schema()
    }
    /// Shared scalar columns in each batch's value order.
    pub fn columns(&self) -> &[Column] {
        self.cursor.columns()
    }
    /// Number of rows successfully returned so far.
    pub fn position(&self) -> u32 {
        self.cursor.position()
    }
    /// Whether a failed fetch has permanently closed continuation.
    pub fn is_failed(&self) -> bool {
        self.cursor.is_failed()
    }
    /// Fetch under the browser's original budget and cancellation callback.
    pub fn next_page(&mut self, limit: u32) -> Result<TableBatch, InspectError> {
        self.cursor.next_page(limit, self.budget, self.cancelled)
    }
}
impl<R: Read + Seek> NativeBrowser<'_, R> {
    /// Open a resumable local scalar cursor without resetting host controls.
    /// Requires a stable/offline source; this does not acquire a native lock.
    pub fn cursor(&mut self, table: &str) -> Result<BrowserCursor<'_, R>, InspectError> {
        let cursor = TableCursor::open(
            &mut self.source,
            self.scope,
            table,
            &mut self.budget,
            self.cancelled,
        )?;
        Ok(BrowserCursor {
            cursor,
            budget: &mut self.budget,
            cancelled: self.cancelled,
        })
    }
}
