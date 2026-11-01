//! Original complete rows for one represented EOF INSERT, never a permission bit.
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::{RecoverableNativeEofInsertPlan, RecoverableNativeInsertOperation};
pub use recovery::{QualifiedNativeEofInsertRecovery, qualify_native_eof_insert_recovery};

use super::{
    NativeIntentError,
    complete_insert::{CompleteRows, Protocol},
};
use crate::RowInsert;
use tablecraft_core::limits::ResourceBudget;

/// Closed TCNA0003/profile3 original-complete-row allocation intent.
/// Omitted/default requests have a separate body and cannot enter this type.
pub struct NativeEofInsertIntent {
    original: CompleteRows,
}

impl NativeEofInsertIntent {
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
        let _span = tracing::debug_span!("native.eof_insert_intent.capture").entered();
        Ok(Self {
            original: CompleteRows::new(table, rows, before, budget, cancelled)?,
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

    /// Encode only the closed profile3 record, without allocating DATA.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, NativeIntentError> {
        self.original.encode(Protocol::Eof, budget, cancelled)
    }

    /// Reject TCNA1/2 and every unknown version/profile before decoded copies.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Ok(Self {
            original: CompleteRows::decode(raw, Protocol::Eof, budget, cancelled)?,
        })
    }
}
