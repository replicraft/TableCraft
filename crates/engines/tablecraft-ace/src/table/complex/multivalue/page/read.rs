use super::super::super::page::{CollectionOptions, read_with_options};
use super::MultivalueTablePage;
use crate::{InspectError, TablePageRequest};
use std::io::{Read, Seek};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Browse all columns with explicit multivalue support and optional cached results.
/// Other collection types fail even on empty pages. Parent/child snapshots retain
/// complete reference/capacity validation; unselected scalar payloads are unchecked.
/// No lookup, formula, provider or active content executes. Use a stable source.
pub fn read_multivalue_table_page<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    request: TablePageRequest,
    include_stored_calculations: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueTablePage, InspectError> {
    let page = read_with_options(
        source,
        scope,
        table,
        request,
        CollectionOptions {
            include_stored_calculations,
            all_collections: false,
        },
        budget,
        cancelled,
    )?;
    Ok(MultivalueTablePage { page })
}
