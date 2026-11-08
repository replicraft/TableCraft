//! Original empty-table ADD-column requests, never serialized save permission.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{AddColumnCandidate, AddColumnError, InspectError, header::check};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub use planning::RecoverableAddColumnPlan;
pub use recovery::{QualifiedAddColumnRecovery, qualify_native_add_column_recovery};

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Refusal before an ADD-column request gains persistence or recovery authority.
#[derive(Debug, Error)]
pub enum AddColumnIntentError {
    /// Native capture, work, allocation or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Full-source digest binding failed.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The actual source or new declaration failed native qualification.
    #[error(transparent)]
    Column(#[from] AddColumnError),
    /// The closed request language or bounded name framing is invalid.
    #[error("invalid add-column intent record")]
    InvalidRecord,
    /// The supplied complete original differs from the captured source.
    #[error("add-column before image does not match")]
    BeforeMismatch,
    /// The shared complete-image proof failed outside a direct mismatch.
    #[error(transparent)]
    Proof(#[from] crate::NativeRecoveryQualificationError),
    /// Fresh reconstruction differs from the retained candidate.
    #[error("add-column reconstruction differs from the complete candidate")]
    ImageMismatch,
}

impl From<LimitError> for AddColumnIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Data-only original request for one nullable ordinary Int32 declaration.
///
/// TCAC0001/profile 1 binds the complete canonical source, exact table spelling
/// and proposed field name. It grants neither write permission nor admission for
/// other field types, populated tables, indexes, defaults or required programs.
pub struct NativeAddColumnIntent {
    table: SchemaName,
    column: SchemaName,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativeAddColumnIntent {
    /// Upper framing bound before a host allocates its retained request buffer.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    fn capture(
        candidate: &AddColumnCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddColumnIntentError> {
        let table = copy_name(candidate.table(), budget, cancelled)?;
        let column = copy_name(candidate.column(), budget, cancelled)?;
        let before_binding = binding::digest(candidate.before_bytes(), budget, cancelled)?;
        check(budget, cancelled)?;

        Ok(Self {
            table,
            column,
            before_length: candidate.before_bytes().len() as u64,
            before_binding,
        })
    }

    /// Exact original decoded table name.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact proposed field name; native validation is repeated during replay.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Encode data bound to an already qualified candidate, without save authority.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, AddColumnIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Refuse malformed framing and both borrowed names before allocating either.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddColumnIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Check complete original binding; this alone does not qualify native writes.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), AddColumnIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length
            || binding::digest(bytes, budget, cancelled)? != self.before_binding
        {
            return Err(AddColumnIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddColumnIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(AddColumnIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(AddColumnIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, AddColumnIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| AddColumnIntentError::InvalidRecord)
}

#[cfg(test)]
mod tests;
