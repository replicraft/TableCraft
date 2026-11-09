//! Closed original saved-field DROP requests, separate from native authority.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

use super::{NativeIntentError, binding};
use crate::{
    FieldDropError, InspectError, NativeFieldDropCandidate, NativeFieldDropIdentity, NativeImage,
    NativeRecoveryQualificationError, header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

pub use planning::RecoverableFieldDropPlan;
pub use recovery::{QualifiedFieldDropRecovery, qualify_native_field_drop_recovery};

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS;

/// Payload-free original DROP, source reconstruction or complete-image failure.
#[derive(Debug, Error)]
pub enum FieldDropIntentError {
    /// Native parsing, cancellation, allocation or cumulative limit failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete original-image binding failed.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The selected source, field or property cleanup is not qualified.
    #[error(transparent)]
    Drop(#[from] FieldDropError),
    /// Complete original or replacement image proof failed.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Closed family/version/profile, framing or bounded text is invalid.
    #[error("invalid native saved-field DROP intent record")]
    InvalidRecord,
    /// Complete original length or source binding differs.
    #[error("native saved-field DROP original image does not match")]
    BeforeMismatch,
    /// Actual native identity or original table/field spelling differs.
    #[error("native saved-field DROP request identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for FieldDropIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Native source facts only; caller CatalogScope and physical spans are absent.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_table: i32,
    definition: u32,
    number: u16,
    column_id: u16,
}

impl Identity {
    fn from_native(identity: NativeFieldDropIdentity) -> Result<Self, FieldDropIntentError> {
        Ok(Self {
            native_table: identity.native_table_id(),
            definition: identity.definition_page(),
            number: identity
                .column_number()
                .try_into()
                .map_err(|_| FieldDropIntentError::InvalidRecord)?,
            column_id: identity.column_id(),
        })
    }
}

/// Data-only original action and complete BEFORE binding. Decode grants no
/// source, property cleanup, replacement, journal or recovery permission.
pub struct NativeFieldDropIntent {
    identity: Identity,
    table: String,
    column: String,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeFieldDropIntent {
    /// Closed upper bound checked before allocating persisted request strings.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        candidate: &NativeFieldDropCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        validate_request(candidate.table(), candidate.column(), budget, cancelled)?;
        let identity = Identity::from_native(candidate.identity())?;
        let before_length = candidate.before_bytes().len() as u64;
        codec::validate_identity(identity, before_length)?;
        let before_binding = binding::digest(candidate.before_bytes(), budget, cancelled)?;
        Ok(Self {
            identity,
            table: copy_text(candidate.table(), budget)?,
            column: copy_text(candidate.column(), budget)?,
            before_length,
            before_binding,
        })
    }

    /// Exact original native table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact original field spelling, not a Caption or visible ordinal.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Native logical number, distinct from current declaration position.
    pub const fn column_number(&self) -> usize {
        self.identity.number as usize
    }

    /// Original stored creation ID, retained independently of native number.
    pub const fn column_id(&self) -> u16 {
        self.identity.column_id
    }

    /// Encode only the original DROP request and complete BEFORE binding.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, FieldDropIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Reject all other action families, versions and profiles without retry.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify complete original bytes before native source reconstruction.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldDropIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest(before, budget, cancelled)? != self.before_binding
        {
            return Err(FieldDropIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldDropCandidate, FieldDropIntentError> {
        let candidate =
            image.prepare_saved_field_drop(self.table(), self.column(), budget, cancelled)?;
        if Identity::from_native(candidate.identity())? != self.identity
            || candidate.table() != self.table()
            || candidate.column() != self.column()
        {
            return Err(FieldDropIntentError::IdentityMismatch);
        }
        Ok(candidate)
    }
}

fn validate_request(
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldDropIntentError> {
    // Profile 1 is explicitly ASCII; byte ceilings precede traversal/allocation.
    for name in [table, column] {
        if name.len() > MAX_NAME_BYTES
            || !name.is_ascii()
            || !crate::identifier::valid_name(name, budget, cancelled)?
        {
            return Err(FieldDropIntentError::InvalidRecord);
        }
    }
    Ok(())
}

fn copy_text(text: &str, budget: &mut ResourceBudget) -> Result<String, FieldDropIntentError> {
    budget.charge_allocation(text.len() as u64, 1)?;
    budget.charge(Resource::Steps, text.len() as u64)?;
    let mut output = String::new();
    output
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(text);
    Ok(output)
}
