//! Per-command native work allowances, independent from query and display quotas.
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod update;
#[cfg(all(
    feature = "offline-edit",
    any(target_os = "macos", target_os = "linux", windows)
))]
pub(crate) use update::{
    update_persistence_budget, update_proof_budget, update_query_budget, update_recovery_budget,
    update_review_budget,
};

use super::Error;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const NATIVE_BYTES: u64 = 512 * 1024 * 1024;
const NATIVE_ROWS: u64 = 100_000;
const NATIVE_STEPS: u64 = 5_000_000;
const COMPARISON_BYTES: u64 = 16 * 1024 * 1024;
const COMPARISON_STEPS: u64 = 1_000_000;
pub(super) const SCOPE: CatalogScope = CatalogScope::new(1);

pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(NATIVE_BYTES, NATIVE_ROWS, NATIVE_STEPS)
}

pub(super) fn comparison_budget() -> ResourceBudget {
    // Comparison providers consume bytes and work, independently of native I/O
    // and query row admission. They do not materialize source records.
    ResourceBudget::new(COMPARISON_BYTES, 0, COMPARISON_STEPS)
}

pub(super) fn check(cancelled: &dyn Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
