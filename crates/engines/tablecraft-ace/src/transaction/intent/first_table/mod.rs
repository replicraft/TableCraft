//! Closed original schema data for canonical blank initialization, never permission.
mod codec;
mod recovery;
mod write;

use super::{NativeIntentError, binding};
use crate::{
    CreateDatabaseError, FirstTableDefinition, FirstTablePlan, InspectError, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

pub use recovery::{QualifiedFirstTableRecovery, qualify_native_first_table_recovery};
pub use write::PreparedFirstTable;

/// Bounded schema-intent framing, declaration or source-binding failure.
#[derive(Debug, Error)]
pub enum FirstTableIntentError {
    /// Resource, allocation or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Original native declarations are outside the constructive profile.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// Exact source binding could not be captured or verified.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Framing, names, tags, flags or reserved bytes are malformed.
    #[error("invalid first-table intent record")]
    InvalidRecord,
    /// The closed schema-intent version is not implemented.
    #[error("first-table intent version is unsupported")]
    UnsupportedVersion,
    /// The pinned construction profile is not implemented.
    #[error("first-table construction profile is unsupported")]
    UnsupportedProfile,
    /// The original full-image binding does not match.
    #[error("first-table intent before image does not match")]
    BeforeMismatch,
}

/// Original owned declarations and full-source binding for TCNT0001/profile 1,
/// default-bearing TCNT0002/profile 2 or explicit-policy TCNT0003/profile 3.
/// Decoding this record supplies no native write, recovery or absence authority.
pub struct NativeFirstTableIntent {
    definition: FirstTableDefinition,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeFirstTableIntent {
    /// Maximum complete record, checked before decoded name/vector allocations.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Decode only the closed original-schema record, with no decoder fallback.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FirstTableIntentError> {
        codec::decode(raw, budget, cancelled)
    }

    /// Exact original table name, not a target path or SQL fragment.
    pub fn table(&self) -> &str {
        self.definition.table()
    }

    /// Number of original field declarations, not changed records.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }

    fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FirstTableIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(FirstTableIntentError::BeforeMismatch);
        }
        let actual = binding::digest(before, budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(FirstTableIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

impl FirstTablePlan {
    /// Encode original names/declarations and exact BEFORE binding only.
    /// Physical offsets, maps, replacement bytes and write authority are absent.
    pub fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, FirstTableIntentError> {
        codec::encode(self.definition(), self.before_bytes(), budget, cancelled)
    }
}

#[cfg(test)]
mod tests;
