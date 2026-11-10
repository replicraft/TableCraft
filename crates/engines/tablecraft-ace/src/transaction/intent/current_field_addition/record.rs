//! Original fixed Number request; serialized data grants no native authority.
use super::{
    super::{NativeIntentError, binding},
    codec,
};
use crate::{
    CurrentFieldAdditionError, FieldAdditionTarget, InspectError,
    NativeCurrentFieldAdditionCandidate, NativeImage, header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub(super) const BINDING_BYTES: usize = binding::DIGEST_BYTES;
const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Refusal before a typed field-addition request acquires persistence authority.
#[derive(Debug, Error)]
pub enum CurrentFieldAdditionIntentError {
    /// Native inspection, cumulative limits, allocation or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The complete original image could not be bound.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The separate current metadata/ownership and fixed declaration proof refused.
    #[error(transparent)]
    Field(#[from] CurrentFieldAdditionError),
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

impl From<LimitError> for CurrentFieldAdditionIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Closed TCAC0008/profile8 original request, never a serialized write permission.
/// Number means nullable LONG, stored Required(false), typed Int32 zero.
/// The original complete image binds every native origin and field identity.
/// Actual current ownership and complete candidate images are requalified.
pub struct NativeCurrentFieldAdditionIntent {
    pub(super) table: SchemaName,
    pub(super) column: SchemaName,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; BINDING_BYTES],
}

impl NativeCurrentFieldAdditionIntent {
    /// Bound checked before a persisted request buffer or names are retained.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    pub(super) fn capture(
        candidate: &NativeCurrentFieldAdditionCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        Ok(Self {
            table: copy_name(candidate.table(), budget, cancelled)?,
            column: copy_name(candidate.column(), budget, cancelled)?,
            before_length: candidate.before_bytes().len() as u64,
            before_binding: binding::digest(candidate.before_bytes(), budget, cancelled)?,
        })
    }

    pub(super) fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentFieldAdditionCandidate, CurrentFieldAdditionIntentError> {
        let candidate = image.prepare_current_number_field_addition(
            self.table(),
            self.column(),
            budget,
            cancelled,
        )?;
        if candidate.table() != self.table() || candidate.column() != self.column() {
            return Err(CurrentFieldAdditionIntentError::IdentityMismatch);
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

    /// Fixed nullable LONG declaration with stored Required(false)/DefaultValue0.
    pub const fn target(&self) -> FieldAdditionTarget {
        FieldAdditionTarget::NumberLongZero
    }

    /// Encode the original request and complete BEFORE binding, never an AFTER.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, CurrentFieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only TCAC0008/profile8 and its fixed Number declaration before allocation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify exact complete original bytes before any native reconstruction.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CurrentFieldAdditionIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length
            || binding::digest(bytes, budget, cancelled)? != self.before_binding
        {
            return Err(CurrentFieldAdditionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

pub(super) fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurrentFieldAdditionIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES || !text.is_ascii() {
        return Err(CurrentFieldAdditionIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(CurrentFieldAdditionIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, CurrentFieldAdditionIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| CurrentFieldAdditionIntentError::InvalidRecord)
}
