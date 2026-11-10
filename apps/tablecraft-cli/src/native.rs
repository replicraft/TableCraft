//! Native inspection/retrieval with optional caller-selected resource budgets.
pub(crate) mod publication;
mod source;
pub(crate) use source::{NativeFile, open, open_raw};

use std::path::Path;
use tablecraft_ace::Header;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_formats::template::{TemplateError, TemplatePackage};
use tablecraft_io::SharedFile;
use tablecraft_model::CatalogScope;

// Root source namespace; explicit linked sources receive distinct invocation-local scopes.
pub(crate) const CATALOG_SCOPE: CatalogScope = CatalogScope::new(1);
pub(crate) const DEFAULT_NATIVE_BYTES: u64 = u64::MAX;
pub(crate) const MAX_QUERY_READ_BYTES: u64 = u64::MAX;
pub(crate) const MAX_NATIVE_READ_ROWS: u64 = u64::MAX;
pub(crate) const MAX_NATIVE_READ_STEPS: u64 = u64::MAX;
pub(crate) const DEFAULT_NATIVE_ROWS: u64 = MAX_NATIVE_READ_ROWS;
pub(crate) const DEFAULT_NATIVE_STEPS: u64 = u64::MAX;
const MAX_HEADER_STEPS: u64 = 10;
/// A cumulative pool spans tables/scans; one snapshot retains u32 row ordinals.
/// Narrow only the snapshot representation, preserving the caller's full pool.
pub(crate) fn snapshot_rows(allowance: u64) -> u32 {
    allowance.min(u64::from(u32::MAX)) as u32
}

pub(crate) fn budget() -> ResourceBudget {
    read_budget(DEFAULT_NATIVE_BYTES)
}
pub(crate) fn read_budget(bytes: u64) -> ResourceBudget {
    ResourceBudget::new(bytes, DEFAULT_NATIVE_ROWS, DEFAULT_NATIVE_STEPS)
}
pub(crate) fn header_budget() -> ResourceBudget {
    ResourceBudget::new(Header::PREFIX_BYTES as u64, 0, MAX_HEADER_STEPS)
}

pub(crate) fn open_template(
    path: &Path,
    budget: &mut ResourceBudget,
) -> Result<Option<TemplatePackage>, TemplateError> {
    let mut file = SharedFile::open(path).map_err(tablecraft_formats::ReadError::from)?;
    TemplatePackage::open_if_template(&mut file, budget, &|| false)
}
