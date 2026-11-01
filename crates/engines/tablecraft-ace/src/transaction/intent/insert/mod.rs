//! Ordered typed insertion requests, distinct from the UPDATE intent family.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeInsertPlan;
pub use recovery::{QualifiedNativeInsertRecovery, qualify_native_insert_recovery};

use super::{NativeIntentError, binding, model as names};
use crate::{InspectError, RowInsert, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

/// Original ordered complete insertion rows and an exact complete BEFORE binding.
/// This data has no public conversion into a native plan or absence permission.
pub struct NativeInsertIntent {
    table: SchemaName,
    rows: Vec<RowInsert>,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeInsertIntent {
    /// Maximum complete TCNA0001 record accepted before host allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Validate and retain original requests in their written order.
    /// Actual target eligibility, row width and final constraints are
    /// independently checked by the normal planner, never granted here.
    pub fn new(
        table: &str,
        rows: Vec<RowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.insert_intent.capture").entered();
        check(budget, cancelled)?;
        names::validate_name(table, budget, cancelled)?;
        codec::validate_requests(table.len(), &rows, budget, cancelled)?;
        let before_binding = binding::digest(before, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Canonical caller-supplied table name without implicit normalization.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact supplied rows, including an empty batch and written order.
    pub fn rows(&self) -> &[RowInsert] {
        &self.rows
    }

    /// Verify exact source bytes before any native replay or target effects.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(NativeIntentError::BeforeMismatch);
        }
        let actual = binding::digest(before, budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(NativeIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}
