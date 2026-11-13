//! Semantic QueryDef originals bound to complete immutable BEFORE bytes.
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

use super::{NativeIntentError, binding};
use crate::{InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use crate::{
    NativeImage, QueryCreationDefinition, QueryCreationError,
    queries::creation::effect::{NativeQueryCreationEffect, NativeQueryCreationParts},
};
use tablecraft_core::{
    limits::{LimitError, ResourceBudget},
    values::LegacyDateTime,
};
use thiserror::Error;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableNativeQueryCreationPlan;
pub use recovery::QualifiedNativeQueryCreationRecovery;

/// Data framing, complete source binding, or fresh reconstruction was refused.
#[derive(Debug, Error)]
pub enum QueryCreationIntentError {
    /// Native inspection, allocation, cancellation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete native source length or binding representation is invalid.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The fresh native candidate rejected its declaration, source or proof.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Native(#[from] QueryCreationError),
    /// A complete checked image differs or independent proof resources failed.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Wrong family/profile, reserved bytes, names, date or identity representation.
    #[error("invalid native query creation original")]
    InvalidRecord,
    /// A recognized creation prefix selects an unsupported version.
    #[error("native query creation original version is unsupported")]
    UnsupportedVersion,
    /// The native constructive implementation is unavailable in this build.
    #[error("native query creation reconstruction is unavailable")]
    UnsupportedSource,
    /// Complete original length or digest differs from the captured source.
    #[error("native query creation original image differs")]
    BeforeMismatch,
    /// Allocated identity differs from fresh complete-source selection.
    #[error("native query creation identity differs")]
    IdentityMismatch,
}

impl From<LimitError> for QueryCreationIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Closed TCQC0001 data; decoding grants no native mutation or recovery authority.
pub struct NativeQueryCreationIntent {
    name: String,
    alias: String,
    value: i32,
    captured_at: LegacyDateTime,
    native_id: i32,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeQueryCreationIntent {
    /// Wire envelope checked before persisted names are allocated.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;

    /// Decode only TCQC0001/profile1 after complete borrowed preflight.
    ///
    /// # Errors
    /// Refuses framing, native facts, names, allocation, cancellation and budgets.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, QueryCreationIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Encode the semantic request, never SQL or generated native record bytes.
    ///
    /// # Errors
    /// Refuses invalid representation, allocation, cancellation or caller limits.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, QueryCreationIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Exact new destination spelling.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Explicit result alias, separate from literal expression syntax.
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Captured signed LONG literal, never an evaluated result.
    pub const fn value(&self) -> i32 {
        self.value
    }

    /// Exact caller-captured DATE fact for both new catalog date fields.
    pub const fn captured_at(&self) -> LegacyDateTime {
        self.captured_at
    }

    /// Signed catalog identity, independently rederived on reconstruction.
    pub const fn native_query_id(&self) -> i32 {
        self.native_id
    }

    /// Complete original size under native retained-file geometry.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Verify the complete source before any constructive reconstruction.
    ///
    /// # Errors
    /// Refuses changed length/digest, native bounds, cancellation or resources.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), QueryCreationIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest_retained(before, budget, cancelled)? != self.before_binding
        {
            return Err(QueryCreationIntentError::BeforeMismatch);
        }
        Ok(())
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        parts: NativeQueryCreationParts,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeQueryCreationEffect, Self), QueryCreationIntentError> {
        check(budget, cancelled)?;
        let NativeQueryCreationParts { definition, effect } = parts;
        let (name, alias, value, captured_at) = definition.into_parts();
        let native_id = effect.native_query_id();
        let before_length = effect.before_bytes().len() as u64;
        codec::validate([&name, &alias], native_id, before_length, budget, cancelled)?;
        let before_binding = binding::digest_retained(effect.before_bytes(), budget, cancelled)?;

        Ok((
            effect,
            Self {
                name,
                alias,
                value,
                captured_at,
                native_id,
                before_length,
                before_binding,
            },
        ))
    }

    #[cfg(feature = "ace16-create")]
    fn reconstruct(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeQueryCreationEffect, Self), QueryCreationIntentError> {
        self.verify_before(image.as_bytes(), budget, cancelled)?;
        let Self {
            name,
            alias,
            value,
            captured_at,
            native_id,
            before_length,
            before_binding,
        } = self;
        let definition = QueryCreationDefinition::from_owned(
            name,
            alias,
            value,
            captured_at,
            budget,
            cancelled,
        )?;

        let candidate = image.prepare_query_creation(definition, budget, cancelled)?;
        if candidate.native_query_id() != native_id {
            return Err(QueryCreationIntentError::IdentityMismatch);
        }

        let NativeQueryCreationParts { definition, effect } = candidate.into_creation_parts();
        let (name, alias, value, captured_at) = definition.into_parts();
        check(budget, cancelled)?;
        Ok((
            effect,
            Self {
                name,
                alias,
                value,
                captured_at,
                native_id,
                before_length,
                before_binding,
            },
        ))
    }
}
