//! Explicit native inspection/retrieval host policy; not native format maxima.

use tablecraft_ace::Header;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

// One source per invocation; this namespace is never persisted as a file identity.
pub(crate) const CATALOG_SCOPE: CatalogScope = CatalogScope::new(1);
pub(crate) const DEFAULT_NATIVE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_QUERY_READ_BYTES: u64 = 256 * 1024 * 1024;
const MAX_NATIVE_ROWS: u64 = 16_384;
const MAX_NATIVE_STEPS: u64 = 1_000_000;
const MAX_HEADER_STEPS: u64 = 10;
pub(crate) fn budget() -> ResourceBudget {
    read_budget(DEFAULT_NATIVE_BYTES)
}
pub(crate) fn read_budget(bytes: u64) -> ResourceBudget {
    ResourceBudget::new(bytes, MAX_NATIVE_ROWS, MAX_NATIVE_STEPS)
}
pub(crate) fn header_budget() -> ResourceBudget {
    ResourceBudget::new(Header::PREFIX_BYTES as u64, 0, MAX_HEADER_STEPS)
}
