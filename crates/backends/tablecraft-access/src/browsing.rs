//! Host-independent native discovery and stored-value browsing.
mod automatic;
pub use automatic::BrowsePage;
mod cursor;
mod description;
pub use cursor::{BrowserCursor, TableBatch};
pub use description::TableDescription;
pub use tablecraft_ace::NativeTableStorage;
pub use tablecraft_ace::StoredTablePage;
mod collections;
pub use collections::{
    AttachmentColumn, CollectionPageRequest, StoredAttachment, StoredCollectionColumn,
    StoredCollectionItem, StoredCollectionPageCell, StoredCollectionPageValue,
    StoredCollectionTablePage, StoredVersion, VersionHistoryColumn,
};
use tablecraft_ace::{CatalogInspection, InspectError, NativeObjectOrigin, inspect_catalog};
pub use tablecraft_ace::{MultivaluePageCell, MultivaluePageValue, MultivalueTablePage};
pub use tablecraft_ace::{TablePage, TablePageRequest};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::{CatalogScope, ObjectDescriptor, ObjectKind};

/// Inert native table discovery, including hidden/system/linked declarations.
/// A descriptor's presence does not establish that its rows are readable.
pub struct TableInventory {
    catalog: CatalogInspection,
}
impl TableInventory {
    /// Table descriptors in captured catalog order; no provider is contacted.
    pub fn tables(&self) -> impl Iterator<Item = &ObjectDescriptor> {
        self.entries().map(|entry| entry.object)
    }
    /// Descriptors paired with storage declarations, without row/schema I/O.
    pub fn entries(&self) -> impl Iterator<Item = TableEntry<'_>> {
        self.catalog
            .catalog()
            .objects()
            .iter()
            .zip(self.catalog.origins())
            .filter(|(object, _)| object.kind() == ObjectKind::Table)
            .map(|(object, origin)| TableEntry { object, origin })
    }
}

/// One table descriptor and its captured native storage declaration.
pub struct TableEntry<'a> {
    object: &'a ObjectDescriptor,
    origin: &'a NativeObjectOrigin,
}
impl<'a> TableEntry<'a> {
    /// Shared object identity/name, borrowed from this inventory.
    pub const fn object(&self) -> &'a ObjectDescriptor {
        self.object
    }
    /// Recognized storage; None means no classification is available.
    /// Local does not prove supported columns or readable rows.
    pub const fn storage(&self) -> Option<NativeTableStorage> {
        self.origin.table_storage()
    }
    /// Whether native catalog flags mark this as a system object.
    /// False does not establish who created a table or whether it is hidden.
    pub const fn is_system(&self) -> bool {
        self.origin.is_system()
    }
}

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
    /// Inventory table descriptors without opening table records or linked sources.
    pub fn tables(&mut self) -> Result<TableInventory, InspectError> {
        let catalog = inspect_catalog(
            &mut self.source,
            self.scope,
            &mut self.budget,
            self.cancelled,
        )?;
        Ok(TableInventory { catalog })
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
