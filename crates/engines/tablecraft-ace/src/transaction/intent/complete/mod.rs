//! Exact complete original rows; no allocation count or target permission.
use super::{
    NativeIntentError,
    complete_insert::{CompleteRows, Protocol},
};
use crate::RowInsert;
use tablecraft_core::limits::ResourceBudget;

/// Closed TCNA0004/profile4 original-complete-row intent.
/// Omitted/default requests have a separate body and cannot enter this type.
pub struct NativeCompleteInsertIntent {
    pub(super) original: CompleteRows,
}

impl NativeCompleteInsertIntent {
    /// Maximum complete encoded record checked before host allocation.
    pub const MAX_ENCODED_BYTES: usize = CompleteRows::MAX_ENCODED_BYTES;

    /// Retain ordered exact carriers and the complete original image digest.
    /// This performs no native allocation or target admission.
    pub fn new(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.complete_insert_intent.capture").entered();
        Ok(Self {
            original: CompleteRows::new_complete(table, rows, before, budget, cancelled)?,
        })
    }

    pub(super) fn from_original(original: CompleteRows) -> Self {
        Self { original }
    }

    /// Exact caller-supplied native table name.
    pub fn table(&self) -> &str {
        self.original.table()
    }

    /// Exact original ordered complete rows, never expanded omitted values.
    pub fn rows(&self) -> &[RowInsert] {
        self.original.rows()
    }

    /// Compare original complete bytes before replay and any target effects.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        self.original.verify_before(before, budget, cancelled)
    }

    /// Encode only the closed profile4 record, without allocating DATA.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.original.encode(Protocol::Complete, budget, cancelled)
    }

    /// Reject TCNA1/2/3 and every unknown version/profile before decoded copies.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Ok(Self {
            original: CompleteRows::decode(raw, Protocol::Complete, budget, cancelled)?,
        })
    }
}

#[cfg(test)]
mod tests;

mod planning;
mod recovery;
pub use planning::RecoverableNativeCompleteInsertPlan;
pub use recovery::{
    QualifiedNativeCompleteInsertRecovery, qualify_native_complete_insert_recovery,
};
