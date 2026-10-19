//! Host-independent native discovery and scalar table browsing.
use std::io::{Read, Seek};
use tablecraft_ace::{CatalogInspection, InspectError, inspect_catalog};
pub use tablecraft_ace::{TablePage, TablePageRequest};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectDescriptor, ObjectKind};

/// Inert native table discovery, including hidden/system/linked declarations.
/// A descriptor's presence does not establish that its rows are readable.
pub struct TableInventory {
    catalog: CatalogInspection,
}
impl TableInventory {
    /// Table descriptors in captured catalog order; no provider is contacted.
    pub fn tables(&self) -> impl Iterator<Item = &ObjectDescriptor> {
        self.catalog
            .catalog()
            .objects()
            .iter()
            .filter(|object| object.kind() == ObjectKind::Table)
    }
}

/// Read-only discovery and page operations shared by CLI and future UI hosts.
///
/// The host supplies stable/offline source ownership. Repeated operations share
/// the same cumulative budget and cancellation callback; there is no implicit
/// snapshot, query evaluation, path opening, formatting or active-content execution.
/// Schema and allocation metadata are read anew per operation, without a cache.
pub struct NativeBrowser<'a, R> {
    source: R,
    scope: CatalogScope,
    budget: ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
}
impl<'a, R: Read + Seek> NativeBrowser<'a, R> {
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
}
