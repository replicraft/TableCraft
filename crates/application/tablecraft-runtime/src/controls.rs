//! Per-command native work allowances, independent from query and display quotas.
use super::Error;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const NATIVE_BYTES: u64 = 512 * 1024 * 1024;
const NATIVE_ROWS: u64 = 100_000;
const NATIVE_STEPS: u64 = 5_000_000;
pub(super) const SCOPE: CatalogScope = CatalogScope::new(1);

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS)
}

pub(super) fn check(cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
