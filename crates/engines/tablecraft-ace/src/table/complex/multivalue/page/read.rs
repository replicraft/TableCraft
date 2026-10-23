use super::super::super::page::{CollectionOptions, read_with_options};
use super::MultivalueTablePage;
use crate::{CollectionPageRequest, InspectError, TablePageRequest, TableSnapshotLimits};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Browse all columns with explicit multivalue support and optional cached results.
/// Other collection types fail even on empty pages. Parent/child snapshots retain
/// complete reference/capacity validation; unselected scalar payloads are unchecked.
/// No lookup, formula, provider or active content executes. Use a stable source.
pub fn read_multivalue_table_page<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    request: TablePageRequest,
    include_stored_calculations: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueTablePage, InspectError> {
    read_multivalue_table_page_with_request(
        source,
        scope,
        table,
        CollectionPageRequest::new(request, TableSnapshotLimits::default())
            .with_stored_calculations(include_stored_calculations),
        budget,
        cancelled,
    )
}

/// Read an explicit multivalue page with complete bounded parent/child validation.
/// Other collection kinds remain unsupported; no partial page or scalar placeholder
/// replaces a failed collection read.
pub fn read_multivalue_table_page_with_request<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    request: CollectionPageRequest,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueTablePage, InspectError> {
    let page = read_with_options(
        source,
        scope,
        table,
        request.page(),
        CollectionOptions {
            include_stored_calculations: request.includes_stored_calculations(),
            all_collections: false,
            snapshot_limits: request.snapshot_limits(),
        },
        budget,
        cancelled,
    )?;
    Ok(MultivalueTablePage { page })
}
