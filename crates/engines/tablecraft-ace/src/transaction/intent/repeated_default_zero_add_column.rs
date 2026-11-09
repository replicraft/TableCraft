//! TCAC0004 data binds the distinct repeated Required(false)/zero source profile.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{
    CreatedDefaultZeroAddColumnError, CreatedDefaultZeroColumnRequired, InspectError, NativeImage,
    RepeatedDefaultZeroAddColumnCandidate, header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub use planning::RecoverableRepeatedDefaultZeroAddColumnPlan;
pub use recovery::{
    QualifiedRepeatedDefaultZeroAddColumnRecovery,
    qualify_native_repeated_default_zero_add_column_recovery,
};

const BINDING_BYTES: usize = 32;
const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Refusal before the repeated-property request acquires persistence authority.
#[derive(Debug, Error)]
pub enum RepeatedDefaultZeroAddColumnIntentError {
    /// Native inspection, cumulative limits, allocation or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The complete original image could not be bound.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The separately qualified repeated source/declaration/history refused.
    #[error(transparent)]
    Column(#[from] CreatedDefaultZeroAddColumnError),
    /// Wrong language/profile/declaration, malformed framing or invalid names.
    #[error("invalid repeated explicit-zero add-column intent record")]
    InvalidRecord,
    /// Original complete length or digest differs from the captured source.
    #[error("repeated explicit-zero add-column before image does not match")]
    BeforeMismatch,
    /// Fresh source-resolved table or requested field identity differs.
    #[error("repeated explicit-zero add-column identity does not match")]
    IdentityMismatch,
    /// Complete image comparison failed or exhausted its own proof allowance.
    #[error(transparent)]
    Proof(#[from] crate::NativeRecoveryQualificationError),
    /// Reconstructed complete images differ from the owning candidate/journal.
    #[error("repeated explicit-zero add-column reconstruction differs")]
    ImageMismatch,
}

impl From<LimitError> for RepeatedDefaultZeroAddColumnIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Closed TCAC0004/profile4 original request, never a serialized write permission.
/// It always declares nullable LONG, stored Required(false), and typed Int32 zero.
/// Historical page layout, descriptors and complete images are reconstructed.
pub struct NativeRepeatedDefaultZeroAddColumnIntent {
    table: SchemaName,
    column: SchemaName,
    before_length: u64,
    before_binding: [u8; BINDING_BYTES],
}

impl NativeRepeatedDefaultZeroAddColumnIntent {
    /// Bound checked before a persisted request buffer or names are retained.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    fn capture(
        candidate: &RepeatedDefaultZeroAddColumnCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RepeatedDefaultZeroAddColumnIntentError> {
        Ok(Self {
            table: copy_name(candidate.table(), budget, cancelled)?,
            column: copy_name(candidate.column(), budget, cancelled)?,
            before_length: candidate.before_bytes().len() as u64,
            before_binding: binding::digest(candidate.before_bytes(), budget, cancelled)?,
        })
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RepeatedDefaultZeroAddColumnCandidate, RepeatedDefaultZeroAddColumnIntentError>
    {
        let candidate = image.prepare_repeated_default_zero_int32_column_candidate(
            self.table(),
            self.column(),
            budget,
            cancelled,
        )?;
        if candidate.table() != self.table() || candidate.column() != self.column() {
            return Err(RepeatedDefaultZeroAddColumnIntentError::IdentityMismatch);
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

    /// This separate profile explicitly stores Required(false); absence refuses.
    pub fn required_property(&self) -> CreatedDefaultZeroColumnRequired {
        CreatedDefaultZeroColumnRequired::StoredFalse
    }

    /// Encode the original request and complete BEFORE binding, never an AFTER.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, RepeatedDefaultZeroAddColumnIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only TCAC0004/profile4/stored-false before allocating names.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RepeatedDefaultZeroAddColumnIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify exact complete original bytes before any native reconstruction.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), RepeatedDefaultZeroAddColumnIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length
            || binding::digest(bytes, budget, cancelled)? != self.before_binding
        {
            return Err(RepeatedDefaultZeroAddColumnIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), RepeatedDefaultZeroAddColumnIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(RepeatedDefaultZeroAddColumnIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(RepeatedDefaultZeroAddColumnIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, RepeatedDefaultZeroAddColumnIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| RepeatedDefaultZeroAddColumnIntentError::InvalidRecord)
}

#[cfg(test)]
mod tests;
