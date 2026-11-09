//! Host-independent native discovery and stored-value browsing.
mod automatic;
mod inventory;
pub use automatic::BrowsePage;
pub use inventory::{ObjectEntry, ObjectInventory};
mod cursor;
mod description;
mod projection;
pub use cursor::BrowserCursor;
pub use description::TableDescription;
mod collections;
use tablecraft_ace::{
    CollectionPageRequest, InspectError, MultivalueTablePage, StoredTablePage, TablePage,
    TablePageRequest,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

/// Read-only discovery and page operations shared by CLI and future UI hosts.
///
/// The host supplies stable/offline source ownership. Repeated operations share
/// the same cumulative budget and cancellation callback; there is no implicit
/// snapshot, query evaluation, path opening, formatting or active-content execution.
/// Independent page operations read metadata anew; a source-bound cursor retains
/// its qualified schema and traversal state until dropped.
pub struct NativeBrowser<'a, R> {
    source: R,
    scope: CatalogScope,
    budget: ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
}
impl<'a, R: tablecraft_ace::NativeReadSource> NativeBrowser<'a, R> {
    /// Capture an explicitly supplied source and controls; performs no I/O yet.
    pub fn new(
        source: R,
        scope: CatalogScope,
        budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            source,
            scope,
            budget,
            cancelled,
        }
    }
    /// Inspect one local table's column support without reading user records.
    /// Linked, malformed and unqualified metadata return their typed engine errors.
    pub fn describe(&mut self, table: &str) -> Result<TableDescription, InspectError> {
        let schema = tablecraft_ace::inspect_table_schema(
            &mut self.source,
            self.scope,
            table,
            &mut self.budget,
            self.cancelled,
        )?;
        if (self.cancelled)() {
            return Err(InspectError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(TableDescription { schema })
    }
    /// Read a bounded page, qualifying all scalar columns before decoding records.
    /// Unselected payloads remain unchecked; use the engine's full audit separately.
    pub fn page(
        &mut self,
        table: &str,
        request: TablePageRequest,
    ) -> Result<TablePage, InspectError> {
        tablecraft_ace::read_table_page(
            &mut self.source,
            self.scope,
            table,
            request,
            &mut self.budget,
            self.cancelled,
        )
    }
    /// Explicit cached-calculation browsing; values may be stale and no formula runs.
    /// Unsupported declarations fail before reading records, even for empty pages.
    pub fn stored_page(
        &mut self,
        table: &str,
        request: TablePageRequest,
    ) -> Result<StoredTablePage, InspectError> {
        tablecraft_ace::read_stored_table_page(
            &mut self.source,
            self.scope,
            table,
            request,
            &mut self.budget,
            self.cancelled,
        )
    }
    /// Explicit stored collections, with optional inert cached scalar results.
    /// Complete parent-key/child snapshots retain their bounded read limits.
    pub fn multivalue_page(
        &mut self,
        table: &str,
        request: TablePageRequest,
        include_stored_calculations: bool,
    ) -> Result<MultivalueTablePage, InspectError> {
        tablecraft_ace::read_multivalue_table_page(
            &mut self.source,
            self.scope,
            table,
            request,
            include_stored_calculations,
            &mut self.budget,
            self.cancelled,
        )
    }

    /// Browse a multivalue parent page with explicit complete collection ceilings.
    /// The browser retains one cumulative budget and publishes no partial page.
    pub fn multivalue_page_with_request(
        &mut self,
        table: &str,
        request: CollectionPageRequest,
    ) -> Result<MultivalueTablePage, InspectError> {
        tablecraft_ace::read_multivalue_table_page_with_request(
            &mut self.source,
            self.scope,
            table,
            request,
            &mut self.budget,
            self.cancelled,
        )
    }
}
