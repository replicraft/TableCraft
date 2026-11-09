//! TCAC0005 binds the original typed field request and complete BEFORE image.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{
    FieldAdditionCandidate, FieldAdditionError, FieldAdditionTarget, InspectError, NativeImage,
    header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub use planning::RecoverableFieldAdditionPlan;
pub use recovery::{QualifiedFieldAdditionRecovery, qualify_native_field_addition_recovery};

const BINDING_BYTES: usize = 32;
const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Refusal before a typed field-addition request acquires persistence authority.
#[derive(Debug, Error)]
pub enum FieldAdditionIntentError {
    /// Native inspection, cumulative limits, allocation or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The complete original image could not be bound.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The separate complete typed source/declaration/history proof refused.
    #[error(transparent)]
    Field(#[from] FieldAdditionError),
    /// Wrong language/profile/declaration, malformed framing or invalid names.
    #[error("invalid typed field-addition intent record")]
    InvalidRecord,
    /// Original complete length or digest differs from the captured source.
    #[error("typed field-addition before image does not match")]
    BeforeMismatch,
    /// Fresh source-resolved table or requested field identity differs.
    #[error("typed field-addition identity does not match")]
    IdentityMismatch,
    /// Complete image comparison failed or exhausted its own proof allowance.
    #[error(transparent)]
    Proof(#[from] crate::NativeRecoveryQualificationError),
    /// Reconstructed complete images differ from the owning candidate/journal.
    #[error("typed field-addition reconstruction differs")]
    ImageMismatch,
}

impl From<LimitError> for FieldAdditionIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Closed TCAC0005/profile5 original request, never a serialized write permission.
/// Number means nullable LONG, stored Required(false), typed Int32 zero.
/// Short Text means sized nullable Text, stored Required(false),
/// AllowZeroLength(true), absent DefaultValue and explicit compression permission.
/// Physical history, descriptors and complete images are reconstructed.
pub struct NativeFieldAdditionIntent {
    table: SchemaName,
    column: SchemaName,
    target: FieldAdditionTarget,
    before_length: u64,
    before_binding: [u8; BINDING_BYTES],
}

impl NativeFieldAdditionIntent {
    /// Bound checked before a persisted request buffer or names are retained.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    fn capture(
        candidate: &FieldAdditionCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldAdditionIntentError> {
        Ok(Self {
            table: copy_name(candidate.table(), budget, cancelled)?,
            column: copy_name(candidate.column(), budget, cancelled)?,
            target: candidate.target(),
            before_length: candidate.before_bytes().len() as u64,
            before_binding: binding::digest(candidate.before_bytes(), budget, cancelled)?,
        })
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionCandidate, FieldAdditionIntentError> {
        let candidate = image.prepare_created_add_field_candidate(
            self.table(),
            self.column(),
            self.target(),
            budget,
            cancelled,
        )?;
        if candidate.table() != self.table()
            || candidate.column() != self.column()
            || candidate.target() != self.target()
        {
            return Err(FieldAdditionIntentError::IdentityMismatch);
        }
        Ok(candidate)
    }

    /// Exact source-resolved table spelling, independent of page addresses.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Complete typed declaration, including exact width and compression choice.
    /// Its variant fixes Required/default/empty-value policies independently of UI.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.target
    }

    /// Encode the original request and complete BEFORE binding, never an AFTER.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, FieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only TCAC0005/profile5 and a closed valid target before name allocation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldAdditionIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify exact complete original bytes before any native reconstruction.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldAdditionIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length
            || binding::digest(bytes, budget, cancelled)? != self.before_binding
        {
            return Err(FieldAdditionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldAdditionIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(FieldAdditionIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(FieldAdditionIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, FieldAdditionIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| FieldAdditionIntentError::InvalidRecord)
}

#[cfg(test)]
mod tests;
