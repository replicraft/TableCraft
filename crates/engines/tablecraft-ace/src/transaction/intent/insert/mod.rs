//! Ordered typed insertion requests, distinct from the UPDATE intent family.
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeInsertPlan;
pub use recovery::{QualifiedNativeInsertRecovery, qualify_native_insert_recovery};

use super::{
    NativeIntentError,
    complete_insert::{CompleteRows, Protocol},
};
use crate::RowInsert;
use tablecraft_core::limits::ResourceBudget;

/// Original complete insertion rows, bound to a fixed-length TCNA0001 operation.
pub struct NativeInsertIntent {
    original: CompleteRows,
}

impl NativeInsertIntent {
    /// Maximum complete fixed-family record before host allocation.
    pub const MAX_ENCODED_BYTES: usize = CompleteRows::MAX_ENCODED_BYTES;

    /// Retain original complete rows with the unchanged TCNA1 binding.
    pub fn new(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.insert_intent.capture").entered();
        Ok(Self {
            original: CompleteRows::new(table, rows, before, budget, cancelled)?,
        })
    }

    pub(super) fn from_original(original: CompleteRows) -> Self {
        Self { original }
    }

    /// Exact supplied native table name.
    pub fn table(&self) -> &str {
        self.original.table()
    }

    /// Original supplied rows in written order.
    pub fn rows(&self) -> &[RowInsert] {
        self.original.rows()
    }

    /// Verify complete immutable source bytes before native replay.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        self.original.verify_before(before, budget, cancelled)
    }

    /// Encode only the closed TCNA1/profile1 record.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.original.encode(Protocol::Existing, budget, cancelled)
    }

    /// Decode only the closed TCNA1/profile1 record before heap copies.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Ok(Self {
            original: CompleteRows::decode(raw, Protocol::Existing, budget, cancelled)?,
        })
    }
}
