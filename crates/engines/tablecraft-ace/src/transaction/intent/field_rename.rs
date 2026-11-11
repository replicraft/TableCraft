//! Closed original field-rename requests, separate from sealed native authority.
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
#[cfg(feature = "ace16-create")]
use crate::{FieldRenameError, NativeFieldRenameIdentity, NativeImage};
use crate::{InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use planning::Candidate;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableFieldRenamePlan;
#[cfg(feature = "ace16-create")]
pub use recovery::{QualifiedFieldRenameRecovery, qualify_native_field_rename_recovery};

#[cfg(all(test, feature = "ace16-create"))]
mod tests;

pub use recovery::QualifiedRetainedFieldRenameRecovery;

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
    #[cfg(feature = "ace16-create")]
    Rename(#[from] FieldRenameError),
    /// This build cannot reconstruct native retained schema edits.
    #[error("native retained schema editing is not enabled")]
    UnsupportedSource,
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
    #[cfg(feature = "ace16-create")]
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    #[cfg(feature = "ace16-create")]
    SameWidth,
    #[cfg(feature = "ace16-create")]
    FittedAscii,
    #[cfg(feature = "ace16-create")]
    PopulatedAscii,
    #[cfg(feature = "ace16-create")]
    CurrentAscii,
    #[cfg(feature = "ace16-create")]
    RetainedAscii,
    RetainedSchemaAscii,
    RetainedTypedAscii,
}

impl Profile {
    const fn tag(self) -> u16 {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::SameWidth => 1,
            #[cfg(feature = "ace16-create")]
            Self::FittedAscii => 2,
            #[cfg(feature = "ace16-create")]
            Self::PopulatedAscii => 3,
            #[cfg(feature = "ace16-create")]
            Self::CurrentAscii => 4,
            #[cfg(feature = "ace16-create")]
            Self::RetainedAscii => 5,
            Self::RetainedSchemaAscii => 6,
            Self::RetainedTypedAscii => 7,
        }
    }

    #[cfg(feature = "ace16-create")]
    fn decode(tag: u16) -> Result<Self, FieldRenameIntentError> {
        match tag {
            1 => Ok(Self::SameWidth),
            2 => Ok(Self::FittedAscii),
            3 => Ok(Self::PopulatedAscii),
            4 => Ok(Self::CurrentAscii),
            5 => Ok(Self::RetainedAscii),
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

    #[cfg(feature = "ace16-create")]
    fn capture(
        profile: Profile,
        candidate: &Candidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldRenameIntentError> {
        let actual = match candidate {
            Candidate::SameWidth(_) => Profile::SameWidth,
            Candidate::FittedAscii(_) => Profile::FittedAscii,
            Candidate::PopulatedAscii(_) => Profile::PopulatedAscii,
            Candidate::CurrentAscii(_) => Profile::CurrentAscii,
            Candidate::RetainedAscii(candidate) if candidate.is_retained_typed() => {
                Profile::RetainedTypedAscii
            }
            Candidate::RetainedAscii(candidate) if candidate.is_retained_schema() => {
                Profile::RetainedSchemaAscii
            }
            Candidate::RetainedAscii(_) => Profile::RetainedAscii,
        };
        if profile != actual {
            return Err(FieldRenameIntentError::InvalidRecord);
        }
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
        codec::validate_identity(identity, before_length, candidate.column().len(), profile)?;
        let before_binding = if matches!(
            profile,
            Profile::RetainedSchemaAscii | Profile::RetainedTypedAscii
        ) {
            binding::digest_retained(candidate.before_bytes(), budget, cancelled)?
        } else {
            binding::digest(candidate.before_bytes(), budget, cancelled)?
        };
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

    /// Whether the original request belongs exclusively to retained schema6.
    pub const fn is_retained_schema(&self) -> bool {
        matches!(self.profile, Profile::RetainedSchemaAscii)
    }

    /// Whether this original belongs exclusively to typed retained TCFR7.
    pub const fn is_retained_typed(&self) -> bool {
        matches!(self.profile, Profile::RetainedTypedAscii)
    }

    /// Decode only TCFR0007/profile7; never retry an older field-rename record.
    pub fn decode_retained_typed(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldRenameIntentError> {
        codec::decode_retained_typed(bytes, budget, cancelled)
    }

    /// Decode only TCFR0006/profile6, without trying the original request decoder.
    pub fn decode_retained_schema(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldRenameIntentError> {
        codec::decode_retained_schema(bytes, budget, cancelled)
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
    #[cfg(feature = "ace16-create")]
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
        if before.len() as u64 != self.before_length {
            return Err(FieldRenameIntentError::BeforeMismatch);
        }
        let actual = if self.is_retained_schema() || self.is_retained_typed() {
            binding::digest_retained(before, budget, cancelled)?
        } else {
            binding::digest(before, budget, cancelled)?
        };
        if actual != self.before_binding {
            return Err(FieldRenameIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    #[cfg(feature = "ace16-create")]
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
            Profile::CurrentAscii => Candidate::CurrentAscii(image.prepare_current_field_rename(
                self.table(),
                self.column(),
                self.new_name(),
                budget,
                cancelled,
            )?),
            Profile::RetainedTypedAscii => {
                Candidate::RetainedAscii(image.prepare_retained_typed_field_rename(
                    self.table(),
                    self.column(),
                    self.new_name(),
                    budget,
                    cancelled,
                )?)
            }
            Profile::RetainedSchemaAscii => {
                Candidate::RetainedAscii(image.prepare_retained_schema_field_rename(
                    self.table(),
                    self.column(),
                    self.new_name(),
                    budget,
                    cancelled,
                )?)
            }
            Profile::RetainedAscii => {
                Candidate::RetainedAscii(image.prepare_retained_field_rename(
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

    #[cfg(feature = "ace16-create")]
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
        || profile.tag() == 1 && replacement.len() != original.len()
        || profile.tag() == 2 && replacement.len() == original.len()
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

#[cfg(test)]
mod retained_tests;
