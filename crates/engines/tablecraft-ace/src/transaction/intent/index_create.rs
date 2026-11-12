//! Original ordinary LONG index request, independent of generated native pages.
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableNativeIndexCreatePlan;
pub use recovery::QualifiedNativeIndexCreateRecovery;

use super::{NativeIntentError, binding};
use crate::{EditError, InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use crate::{NativeImage, NativeLongIndexCandidate, SecondaryIndexCreate};
use tablecraft_core::limits::{LimitError, ResourceBudget};
use thiserror::Error;

/// Original index framing, whole-source binding or fresh native reconstruction failed.
#[derive(Debug, Error)]
pub enum IndexCreateIntentError {
    /// Native inspection, allocation, cancellation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete native source-length or binding representation is invalid.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Native source, selected field, allocation or independent candidate proof refused.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// Fresh complete BEFORE or AFTER differs from its checked journal image.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Wrong family/profile, reserved bits, text or native identity representation.
    #[error("invalid native index-creation original")]
    InvalidRecord,
    /// The complete original length or digest differs.
    #[error("native index-creation original image differs")]
    BeforeMismatch,
    /// Fresh table/field identity differs from the original request witness.
    #[error("native index-creation target identity differs")]
    IdentityMismatch,
    /// This build can decode the original but cannot reconstruct native creation.
    #[error("native index creation is not enabled")]
    UnsupportedSource,
}

impl From<LimitError> for IndexCreateIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_table: i32,
    definition: u32,
    number: u16,
    column_id: u16,
}

/// TCIC0001 original names, direction and native target witness, bound to all BEFORE bytes.
/// Decoded data grants no source certificate, physical page scope or write authority.
pub struct NativeIndexCreateIntent {
    table: String,
    column: String,
    index: String,
    descending: bool,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeIndexCreateIntent {
    /// Fixed framing plus three native names of at most 64 UTF-16 units each.
    /// This UTF-8 envelope does not replace native identifier validation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;

    /// Decode only TCIC0001/profile1. No old mutation family is attempted.
    ///
    /// # Errors
    /// Rejects invalid framing, profile, flags, identity, names, allocation,
    /// cancellation and caller resource exhaustion before publishing an owner.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IndexCreateIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Encode the original request and complete source binding, never generated pages.
    ///
    /// # Errors
    /// Rejects invalid representation, allocation, cancellation or caller limits.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, IndexCreateIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Original resolved table name, without a caller-scoped object capability.
    pub fn table(&self) -> &str {
        &self.table
    }
    /// Original selected field name, independent of display ordinal.
    pub fn column(&self) -> &str {
        &self.column
    }
    /// Original requested index name.
    pub fn index_name(&self) -> &str {
        &self.index
    }
    /// Whether native keys are descending; ROW ties remain ascending.
    pub const fn descending(&self) -> bool {
        self.descending
    }
    /// Original signed native catalog table identity.
    pub const fn native_table_id(&self) -> i32 {
        self.identity.native_table
    }
    /// Original table-definition root page.
    pub const fn definition_page(&self) -> u32 {
        self.identity.definition
    }
    /// Original native field identity, which may be zero for an ordinary field.
    pub const fn native_number(&self) -> u16 {
        self.identity.number
    }
    /// Original creation-time field ID, distinct from native/display position.
    pub const fn column_id(&self) -> u16 {
        self.identity.column_id
    }
    /// Complete original image length under native retained-file geometry.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Verify all original bytes before any native reconstruction or target effect.
    ///
    /// # Errors
    /// Returns BeforeMismatch, invalid native geometry, cancellation or caller limits.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IndexCreateIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(IndexCreateIntentError::BeforeMismatch);
        }
        if binding::digest_retained(before, budget, cancelled)? != self.before_binding {
            return Err(IndexCreateIntentError::BeforeMismatch);
        }
        Ok(())
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        candidate: &NativeLongIndexCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IndexCreateIntentError> {
        if candidate.is_fresh_construction() || candidate.is_unique_construction() {
            return Err(IndexCreateIntentError::InvalidRecord);
        }
        let identity = Identity {
            native_table: candidate.native_table_id(),
            definition: candidate.definition_page(),
            number: candidate
                .native_number()
                .try_into()
                .map_err(|_| IndexCreateIntentError::InvalidRecord)?,
            column_id: candidate.column_id(),
        };
        let before_length = candidate.before_bytes().len() as u64;
        codec::validate(
            identity,
            before_length,
            [
                candidate.table(),
                candidate.column(),
                candidate.index_name(),
            ],
            budget,
            cancelled,
        )?;
        Ok(Self {
            table: codec::copy(candidate.table(), budget)?,
            column: codec::copy(candidate.column(), budget)?,
            index: codec::copy(candidate.index_name(), budget)?,
            descending: candidate.descending(),
            identity,
            before_length,
            before_binding: binding::digest_retained(candidate.before_bytes(), budget, cancelled)?,
        })
    }

    #[cfg(feature = "ace16-create")]
    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeLongIndexCandidate, IndexCreateIntentError> {
        self.verify_before(image.as_bytes(), budget, cancelled)?;
        let candidate = image.prepare_retained_long_index_creation(
            self.table(),
            &SecondaryIndexCreate::new(self.index_name(), self.column(), self.descending()),
            budget,
            cancelled,
        )?;
        let actual = Identity {
            native_table: candidate.native_table_id(),
            definition: candidate.definition_page(),
            number: candidate
                .native_number()
                .try_into()
                .map_err(|_| IndexCreateIntentError::IdentityMismatch)?,
            column_id: candidate.column_id(),
        };
        if actual != self.identity {
            return Err(IndexCreateIntentError::IdentityMismatch);
        }
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests;
