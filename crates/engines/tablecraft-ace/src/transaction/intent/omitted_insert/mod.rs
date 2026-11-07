//! Ordered original supplied/omitted ordinary INSERT, distinct from complete TCNA1.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeOmittedInsertPlan;
pub use recovery::{QualifiedNativeOmittedInsertRecovery, qualify_native_omitted_insert_recovery};

use super::{NativeIntentError, binding, model as names, row_carrier::Profile};
use crate::{InspectError, OmittedRowInsert, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

/// Original ordered supplied/omitted rows and an exact complete BEFORE binding.
/// This data has no public conversion into a native plan or absence permission.
pub struct NativeOmittedInsertIntent {
    table: SchemaName,
    rows: Vec<OmittedRowInsert>,
    before_length: u64,
    before_binding: [u8; 32],
    profile: Profile,
}

impl NativeOmittedInsertIntent {
    /// Maximum complete TCNA0002 record accepted before host allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Validate and retain original requests in their written order.
    /// Actual target eligibility, row width and final constraints are
    /// independently checked by the normal planner, never granted here.
    pub fn new(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.omitted_insert_intent.capture").entered();
        check(budget, cancelled)?;
        names::validate_name(table, budget, cancelled)?;
        let mut profile = Profile::Legacy;
        for row in &rows {
            for field in row.values() {
                check(budget, cancelled)?;
                if let crate::InsertFieldValue::Supplied(value) = field
                    && Profile::for_value(value) == Profile::Wider
                {
                    profile = Profile::Wider;
                }
            }
        }
        codec::validate_requests(table.len(), &rows, profile, budget, cancelled)?;
        let before_binding = binding::digest(before, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: before.len() as u64,
            before_binding,
            profile,
        })
    }

    /// Legacy envelopes must not acquire the wider supplied-carrier grammar.
    pub(super) fn is_legacy_profile(&self) -> bool {
        self.profile == Profile::Legacy
    }

    /// Exact original-presence wire size for a bounded execution-input envelope.
    pub(in crate::transaction::intent) fn encoded_length(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, NativeIntentError> {
        codec::validate_requests(
            self.table().len(),
            self.rows(),
            self.profile,
            budget,
            cancelled,
        )
    }

    /// Canonical caller-supplied table name without implicit normalization.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact original presence rows, including an empty batch and written order.
    pub fn rows(&self) -> &[OmittedRowInsert] {
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
