//! Explicit native inspection/retrieval host policy; not native format maxima.
mod source;
pub(crate) use source::{NativeFile, open, open_raw};

use std::path::Path;
use tablecraft_access::template::{TemplateError, TemplatePackage};
use tablecraft_ace::{Header, InspectError, TableSnapshotLimits};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::SharedFile;
use tablecraft_model::CatalogScope;

// Root source namespace; explicit linked sources receive distinct invocation-local scopes.
pub(crate) const CATALOG_SCOPE: CatalogScope = CatalogScope::new(1);
pub(crate) const DEFAULT_NATIVE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_QUERY_READ_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const MAX_NATIVE_READ_ROWS: u64 = TableSnapshotLimits::MAX_ROWS as u64;
pub(crate) const MAX_NATIVE_READ_STEPS: u64 = 100_000_000;
pub(crate) const DEFAULT_NATIVE_ROWS: u64 = 16_384;
pub(crate) const DEFAULT_NATIVE_STEPS: u64 = 1_000_000;
const MAX_HEADER_STEPS: u64 = 10;
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
    let mut file = SharedFile::open(path).map_err(InspectError::from)?;
    TemplatePackage::open_if_template(&mut file, budget, &|| false)
}
