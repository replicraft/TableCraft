//! Ordered raw deletion requests, distinct from the UPDATE intent family.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeDeletePlan;
pub use recovery::{QualifiedNativeDeleteRecovery, qualify_native_delete_recovery};

use super::{NativeIntentError, binding, model as names};
use crate::{InspectError, RowDelete, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

/// Original ordered deletion witnesses and an exact complete BEFORE binding.
/// This data has no public conversion into a native plan or absence permission.
pub struct NativeDeleteIntent {
    table: SchemaName,
    deletes: Vec<RowDelete>,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeDeleteIntent {
    /// Maximum complete TCND0001 record accepted before host allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Validate and retain original requests in their written order.
    /// Actual raw record framing, target eligibility and stale witnesses are
    /// independently checked by the normal planner, never granted here.
    pub fn new(
        table: &str,
        deletes: Vec<RowDelete>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.delete_intent.capture").entered();
        check(budget, cancelled)?;
        names::validate_name(table, budget, cancelled)?;
        codec::validate_requests(table.len(), &deletes, budget, cancelled)?;
        let before_binding = binding::digest(before, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            deletes,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Canonical caller-supplied table name without implicit normalization.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact original raw requests, including empty selection and written order.
    pub fn deletes(&self) -> &[RowDelete] {
        &self.deletes
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
