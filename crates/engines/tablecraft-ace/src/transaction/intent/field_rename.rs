//! Closed original field-rename requests, separate from sealed native authority.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{
    FieldRenameError, InspectError, NativeFieldRenameIdentity, NativeImage,
    NativeRecoveryQualificationError, header::check,
};
use planning::Candidate;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

pub use planning::RecoverableFieldRenamePlan;
pub use recovery::{QualifiedFieldRenameRecovery, qualify_native_field_rename_recovery};

#[cfg(test)]
mod tests;

const MAX_NAME_UNITS: usize = 64;
const MAX_TABLE_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();
const MAX_FIELD_BYTES: usize = MAX_NAME_UNITS;

/// Payload-free original request, reconstruction or complete-image proof failure.
#[derive(Debug, Error)]
pub enum FieldRenameIntentError {
    /// Native parsing, cancellation, allocation or cumulative resource exhaustion.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete original-image binding could not be validated.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The original source, selected field or proposed spelling is unqualified.
    #[error(transparent)]
    Rename(#[from] FieldRenameError),
    /// Complete original or replacement image proof failed.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Closed family/version/profile, framing or borrowed request is invalid.
    #[error("invalid native field-rename intent record")]
    InvalidRecord,
    /// Exact original length or source binding differs.
    #[error("native field-rename original image does not match")]
    BeforeMismatch,
    /// Native identity or exact original/proposed names differ.
    #[error("native field-rename request identity does not match")]
    IdentityMismatch,
}

impl From<LimitError> for FieldRenameIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Stable native facts; caller CatalogScope and physical span authority are absent.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_table: i32,
    definition: u32,
    number: u16,
    column_id: u16,
    name_units: u16,
}

impl Identity {
    fn from_native(identity: NativeFieldRenameIdentity) -> Result<Self, FieldRenameIntentError> {
        Ok(Self {
            native_table: identity.native_table_id(),
            definition: identity.definition_page(),
            number: identity
                .column_number()
                .try_into()
                .map_err(|_| FieldRenameIntentError::InvalidRecord)?,
            column_id: identity.column_id(),
            name_units: identity
                .name_utf16_units()
                .try_into()
                .map_err(|_| FieldRenameIntentError::InvalidRecord)?,
        })
    }
}

/// Persisted profiles select their own validation and native reconstruction.
/// A malformed older request is never retried through the fitted profile.
#[derive(Clone, Copy)]
enum Profile {
    SameWidth,
    FittedAscii,
    PopulatedAscii,
}

impl Profile {
    const fn tag(self) -> u16 {
        match self {
            Self::SameWidth => 1,
            Self::FittedAscii => 2,
            Self::PopulatedAscii => 3,
        }
    }

    fn decode(tag: u16) -> Result<Self, FieldRenameIntentError> {
        match tag {
            1 => Ok(Self::SameWidth),
            2 => Ok(Self::FittedAscii),
            3 => Ok(Self::PopulatedAscii),
            _ => Err(FieldRenameIntentError::InvalidRecord),
        }
    }
}

/// Bounded raw request and complete BEFORE binding. Decoding grants no native
/// source, replacement-image, journal, recovery or save authority.
pub struct NativeFieldRenameIntent {
    profile: Profile,
    identity: Identity,
    table: String,
    original: String,
    replacement: String,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeFieldRenameIntent {
    /// Closed upper bound, checked before any persisted string allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    fn capture(
        profile: Profile,
        candidate: &Candidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldRenameIntentError> {
        validate_request(
            profile,
            candidate.table(),
            candidate.column(),
            candidate.new_name(),
            budget,
            cancelled,
        )?;
        let identity = Identity::from_native(candidate.identity())?;
        let before_length = candidate.before_bytes().len() as u64;
        codec::validate_identity(identity, before_length, candidate.column().len())?;
        let before_binding = binding::digest(candidate.before_bytes(), budget, cancelled)?;
        Ok(Self {
            profile,
            identity,
            table: copy_text(candidate.table(), budget)?,
            original: copy_text(candidate.column(), budget)?,
            replacement: copy_text(candidate.new_name(), budget)?,
            before_length,
            before_binding,
        })
    }

    /// Actual source-resolved table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact original native field spelling, not a Caption or visible ordinal.
    pub fn column(&self) -> &str {
        &self.original
    }

    /// Exact requested spelling, including an unchanged or case-only request.
    pub fn new_name(&self) -> &str {
        &self.replacement
    }

    /// Actual native logical field number.
    pub const fn column_number(&self) -> usize {
        self.identity.number as usize
    }

    /// Original stored creation ID, distinct from current order.
    pub const fn column_id(&self) -> u16 {
        self.identity.column_id
    }

    /// Encode only the original request and BEFORE binding.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, FieldRenameIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Reject every other family/version/profile without decoder fallback.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldRenameIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify the entire original before reconstructing native source authority.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldRenameIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest(before, budget, cancelled)? != self.before_binding
        {
            return Err(FieldRenameIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Candidate, FieldRenameIntentError> {
        let candidate = match self.profile {
            Profile::SameWidth => Candidate::SameWidth(image.prepare_same_width_field_rename(
                self.table(),
                self.column(),
                self.new_name(),
                budget,
                cancelled,
            )?),
            Profile::FittedAscii => Candidate::FittedAscii(image.prepare_fitted_field_rename(
                self.table(),
                self.column(),
                self.new_name(),
                budget,
                cancelled,
            )?),
            Profile::PopulatedAscii => {
                Candidate::PopulatedAscii(image.prepare_populated_field_rename(
                    self.table(),
                    self.column(),
                    self.new_name(),
                    budget,
                    cancelled,
                )?)
            }
        };
        self.verify_candidate(&candidate)?;
        Ok(candidate)
    }

    fn verify_candidate(&self, candidate: &Candidate) -> Result<(), FieldRenameIntentError> {
        if Identity::from_native(candidate.identity())? != self.identity
            || candidate.table() != self.table()
            || candidate.column() != self.column()
            || candidate.new_name() != self.new_name()
        {
            return Err(FieldRenameIntentError::IdentityMismatch);
        }
        Ok(())
    }
}

fn validate_request(
    profile: Profile,
    table: &str,
    original: &str,
    replacement: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldRenameIntentError> {
    // Byte ceilings precede UTF16 traversal and all three owned allocations.
    if table.len() > MAX_TABLE_BYTES
        || original.len() > MAX_FIELD_BYTES
        || replacement.len() > MAX_FIELD_BYTES
        || matches!(profile, Profile::SameWidth) && replacement.len() != original.len()
        || matches!(profile, Profile::FittedAscii) && replacement.len() == original.len()
        || !original.is_ascii()
        || !replacement.is_ascii()
    {
        return Err(FieldRenameIntentError::InvalidRecord);
    }
    for name in [table, original, replacement] {
        if !crate::identifier::valid_name(name, budget, cancelled)? {
            return Err(FieldRenameIntentError::InvalidRecord);
        }
    }
    Ok(())
}

fn copy_text(text: &str, budget: &mut ResourceBudget) -> Result<String, FieldRenameIntentError> {
    budget.charge_allocation(text.len() as u64, 1)?;
    budget.charge(Resource::Steps, text.len() as u64)?;
    let mut output = String::new();
    output
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    output.push_str(text);
    Ok(output)
}
