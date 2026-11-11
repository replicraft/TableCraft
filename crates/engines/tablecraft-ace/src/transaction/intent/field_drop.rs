//! Closed original saved-field DROP requests, separate from native authority.
#[cfg(feature = "ace16-create")]
mod candidate;
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;

#[cfg(all(test, feature = "ace16-create"))]
mod tests;

use super::{NativeIntentError, binding};
#[cfg(feature = "ace16-create")]
use crate::{FieldDropError, NativeFieldDropIdentity, NativeImage};
use crate::{InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use candidate::Candidate;
use codec::Profile;
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableFieldDropPlan;
#[cfg(feature = "ace16-create")]
pub use recovery::{QualifiedFieldDropRecovery, qualify_native_field_drop_recovery};

pub use recovery::QualifiedRetainedFieldDropRecovery;

const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS;
// A BMP scalar occupies at most three UTF-8 bytes per native UTF-16 unit;
// supplementary scalars occupy four bytes for two units.
const MAX_NON_TAIL_NAME_BYTES: usize = 3 * crate::identifier::MAX_NAME_UNITS;

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
    #[cfg(feature = "ace16-create")]
    Drop(#[from] FieldDropError),
    /// This build cannot reconstruct native retained schema edits.
    #[error("native retained schema editing is not enabled")]
    UnsupportedSource,
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
    #[cfg(feature = "ace16-create")]
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
    profile: Profile,
    identity: Identity,
    table: String,
    column: String,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeFieldDropIntent {
    /// Closed upper bound of the original TCFD1–4 request languages.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Largest TCFD0005 request with two native 64-unit UTF-16 names.
    /// This byte envelope does not replace native identifier validation.
    pub const MAX_NON_TAIL_ENCODED_BYTES: usize = codec::MAX_NON_TAIL_RECORD_BYTES;

    #[cfg(feature = "ace16-create")]
    fn capture(
        candidate: &Candidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        validate_request(
            candidate.table(),
            candidate.column(),
            candidate.profile(),
            budget,
            cancelled,
        )?;
        let identity = Identity::from_native(candidate.identity())?;
        let before_length = candidate.before_bytes().len() as u64;
        codec::validate_identity(identity, before_length, candidate.profile())?;
        let before_binding = if matches!(
            candidate.profile(),
            Profile::RetainedTail | Profile::RetainedTypedTail | Profile::RetainedNonTail
        ) {
            binding::digest_retained(candidate.before_bytes(), budget, cancelled)?
        } else {
            binding::digest(candidate.before_bytes(), budget, cancelled)?
        };
        Ok(Self {
            profile: candidate.profile(),
            identity,
            table: copy_text(candidate.table(), budget)?,
            column: copy_text(candidate.column(), budget)?,
            before_length,
            before_binding,
        })
    }

    /// Whether this request belongs exclusively to the retained schema language.
    pub const fn is_retained(&self) -> bool {
        matches!(self.profile, Profile::RetainedTail)
    }

    /// Whether this original belongs exclusively to typed retained TCFD4.
    pub const fn is_retained_typed(&self) -> bool {
        matches!(self.profile, Profile::RetainedTypedTail)
    }

    /// Whether this original belongs exclusively to non-tail TCFD0005.
    pub const fn is_retained_non_tail(&self) -> bool {
        matches!(self.profile, Profile::RetainedNonTail)
    }

    /// Decode only TCFD0005/profile5. Data alone grants no native authority;
    /// older tail request languages are rejected without retry.
    ///
    /// # Errors
    /// Refuses malformed framing, identities, names or native image lengths,
    /// cancellation, allocation failure and explicit resource exhaustion.
    pub fn decode_retained_non_tail(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, Profile::RetainedNonTail, budget, cancelled)
    }

    /// Decode only TCFD0004/profile4; no older action is retried.
    pub fn decode_retained_typed(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, Profile::RetainedTypedTail, budget, cancelled)
    }

    /// Decode only retained TCFD0003/profile3; no legacy decoder is attempted.
    pub fn decode_retained(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, Profile::RetainedTail, budget, cancelled)
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
    #[cfg(feature = "ace16-create")]
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, Profile::Empty, budget, cancelled)
    }

    /// Decode only TCFD0002/profile2 populated-tail requests before allocation.
    /// The original empty-source decoder remains closed to this language.
    #[cfg(feature = "ace16-create")]
    pub fn decode_current_tail(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldDropIntentError> {
        codec::decode(bytes, Profile::PopulatedTail, budget, cancelled)
    }

    /// Verify complete original bytes before native source reconstruction.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldDropIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(FieldDropIntentError::BeforeMismatch);
        }
        let actual =
            if self.is_retained() || self.is_retained_typed() || self.is_retained_non_tail() {
                binding::digest_retained(before, budget, cancelled)?
            } else {
                binding::digest(before, budget, cancelled)?
            };
        if actual != self.before_binding {
            return Err(FieldDropIntentError::BeforeMismatch);
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
    ) -> Result<Candidate, FieldDropIntentError> {
        let candidate = match self.profile {
            Profile::Empty => Candidate::empty(
                image.prepare_saved_field_drop(self.table(), self.column(), budget, cancelled)?,
                budget,
                cancelled,
            )?,
            Profile::RetainedNonTail => Candidate::retained_non_tail(
                image.prepare_retained_non_tail_field_drop(
                    self.table(),
                    self.column(),
                    budget,
                    cancelled,
                )?,
                budget,
                cancelled,
            )?,
            Profile::RetainedTypedTail => Candidate::populated_tail(
                image.prepare_retained_typed_tail_field_drop(
                    self.table(),
                    self.column(),
                    budget,
                    cancelled,
                )?,
                budget,
                cancelled,
            )?,
            Profile::RetainedTail => Candidate::populated_tail(
                image.prepare_retained_tail_field_drop(
                    self.table(),
                    self.column(),
                    budget,
                    cancelled,
                )?,
                budget,
                cancelled,
            )?,
            Profile::PopulatedTail => Candidate::populated_tail(
                image.prepare_current_tail_field_drop(
                    self.table(),
                    self.column(),
                    budget,
                    cancelled,
                )?,
                budget,
                cancelled,
            )?,
        };
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
    profile: Profile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldDropIntentError> {
    // Older languages remain ASCII. Profile 5 validates actual native UTF-16
    // units; its byte envelope only bounds persisted request allocation.
    for name in [table, column] {
        if name.len() > profile.name_bytes()
            || !matches!(profile, Profile::RetainedNonTail) && !name.is_ascii()
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

#[cfg(test)]
mod retained_tests;

#[cfg(test)]
mod typed_tests;
