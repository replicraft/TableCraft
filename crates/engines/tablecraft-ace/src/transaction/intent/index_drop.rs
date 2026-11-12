//! Original DROP request, independent of retired pages or journal framing.
mod codec;
mod unique;
pub use unique::NativeUniqueLongIndexDropIntent;
#[cfg(feature = "ace16-create")]
pub use unique::RecoverableNativeUniqueLongIndexDropPlan;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableNativeIndexDropPlan;
pub use recovery::QualifiedNativeIndexDropRecovery;

use super::{NativeIntentError, binding};
use crate::{EditError, InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use crate::{
    NativeImage,
    table::{NativeIndexDropEffect, NativeIndexDropParts},
};
use tablecraft_core::limits::{LimitError, ResourceBudget};
use thiserror::Error;

/// Original DROP representation, source binding or native reconstruction failed.
#[derive(Debug, Error)]
pub enum IndexDropIntentError {
    /// Inspection, allocation, cancellation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Native complete-source geometry or binding representation is invalid.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// Complete source, target, ownership or independent effect proof refused.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// Complete native image differs from the immutable expected image.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Family, profile, flags, names or native identity representation is invalid.
    #[error("invalid native index-drop original")]
    InvalidRecord,
    /// Complete original length or digest differs.
    #[error("native index-drop original image differs")]
    BeforeMismatch,
    /// Fresh native table, definition or index identity differs.
    #[error("native index-drop target identity differs")]
    IdentityMismatch,
    /// This feature profile cannot construct the original native DROP request.
    #[error("native index drop is not enabled")]
    UnsupportedSource,
}

impl From<LimitError> for IndexDropIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_table: i32,
    definition: u32,
    logical: u32,
    physical: u32,
}

#[cfg(feature = "ace16-create")]
#[derive(Clone, Copy)]
pub(super) enum CapturePolicy {
    Ordinary,
    UniqueLong,
}

/// Nominal original for the unchanged ordinary source language.
pub struct NativeIndexDropIntent {
    data: OriginalData,
}

/// Inert owned names, native identity and complete original binding.
/// Exact family and source policy belong to the nominal wrappers.
pub(super) struct OriginalData {
    table: String,
    index: String,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl OriginalData {
    /// Fixed 80-byte header plus two native names of at most 64 UTF-16 units each.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;
    /// Decode only TCID0001/profile1 without constructing native authority.
    ///
    /// # Errors
    /// Refuses malformed framing/identity/names, allocation, cancellation or limits.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IndexDropIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Encode the original request and source binding, never a retirement list or SQL.
    ///
    /// # Errors
    /// Refuses invalid representation, cancellation, allocation or caller limits.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, IndexDropIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Original table spelling supplied to the qualified native producer.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Original requested logical index name, separate from its physical aliases.
    pub fn index_name(&self) -> &str {
        &self.index
    }

    /// Original complete signed native catalog identity.
    pub const fn native_table_id(&self) -> i32 {
        self.identity.native_table
    }

    /// Original table-definition root, separate from the removed INDEX root.
    pub const fn definition_page(&self) -> u32 {
        self.identity.definition
    }

    /// Original logical index creation identity; zero is a valid ordinary index.
    pub const fn logical_index_number(&self) -> u32 {
        self.identity.logical
    }

    /// Original physical tree identity; surviving aliases retain that tree.
    pub const fn physical_index_number(&self) -> u32 {
        self.identity.physical
    }

    /// Complete source length under native retained-file geometry.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Check every original byte before native reconstruction or target effects.
    ///
    /// # Errors
    /// Returns BeforeMismatch, native geometry, cancellation or caller limits.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IndexDropIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(IndexDropIntentError::BeforeMismatch);
        }
        if binding::digest_retained(before, budget, cancelled)? != self.before_binding {
            return Err(IndexDropIntentError::BeforeMismatch);
        }
        Ok(())
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        parts: NativeIndexDropParts,
        policy: CapturePolicy,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeIndexDropEffect, Self), IndexDropIntentError> {
        let NativeIndexDropParts {
            effect,
            table,
            index,
        } = parts;
        if effect.is_unique_drop() != matches!(policy, CapturePolicy::UniqueLong) {
            return Err(IndexDropIntentError::InvalidRecord);
        }
        let identity = Identity::from_effect(&effect);
        let before_length = effect.before_bytes().len() as u64;
        codec::validate(identity, before_length, [&table, &index], budget, cancelled)?;
        let before_binding = binding::digest_retained(effect.before_bytes(), budget, cancelled)?;
        Ok((
            effect,
            Self {
                table,
                index,
                identity,
                before_length,
                before_binding,
            },
        ))
    }
}

#[cfg(feature = "ace16-create")]
impl Identity {
    fn from_effect(effect: &NativeIndexDropEffect) -> Self {
        Self {
            native_table: effect.native_table_id(),
            definition: effect.definition_page(),
            logical: effect.logical_index_number(),
            physical: effect.physical_index_number(),
        }
    }
}

#[cfg(test)]
mod tests;

impl NativeIndexDropIntent {
    /// Maximum closed name/identity wire envelope.
    pub const MAX_ENCODED_BYTES: usize = OriginalData::MAX_ENCODED_BYTES;

    /// Decode only TCID0001/profile1 without constructing native authority.
    ///
    /// # Errors
    /// Refuses malformed framing/identity/names, allocation, cancellation or limits.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IndexDropIntentError> {
        OriginalData::decode(bytes, budget, cancelled).map(|data| Self { data })
    }

    /// Encode the original request and source binding, never a retirement list or SQL.
    ///
    /// # Errors
    /// Refuses invalid representation, cancellation, allocation or caller limits.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, IndexDropIntentError> {
        self.data.encode(budget, cancelled)
    }

    /// Check every original byte before native reconstruction or target effects.
    ///
    /// # Errors
    /// Returns BeforeMismatch, native geometry, cancellation or caller limits.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IndexDropIntentError> {
        self.data.verify_before(bytes, budget, cancelled)
    }

    /// Original table spelling supplied to the qualified native producer.
    pub fn table(&self) -> &str {
        self.data.table()
    }

    /// Original requested logical index name, separate from its physical aliases.
    pub fn index_name(&self) -> &str {
        self.data.index_name()
    }

    /// Original complete signed native catalog identity.
    pub const fn native_table_id(&self) -> i32 {
        self.data.native_table_id()
    }

    /// Original table-definition root, separate from the removed INDEX root.
    pub const fn definition_page(&self) -> u32 {
        self.data.definition_page()
    }

    /// Original logical index creation identity; zero is a valid ordinary index.
    pub const fn logical_index_number(&self) -> u32 {
        self.data.logical_index_number()
    }

    /// Original physical tree identity; surviving aliases retain that tree.
    pub const fn physical_index_number(&self) -> u32 {
        self.data.physical_index_number()
    }

    /// Complete source length under native retained-file geometry.
    pub const fn before_length(&self) -> u64 {
        self.data.before_length()
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        parts: NativeIndexDropParts,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeIndexDropEffect, Self), IndexDropIntentError> {
        OriginalData::capture(parts, CapturePolicy::Ordinary, budget, cancelled)
            .map(|(effect, data)| (effect, Self { data }))
    }

    #[cfg(feature = "ace16-create")]
    fn reconstruct(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeIndexDropEffect, Self), IndexDropIntentError> {
        self.verify_before(image.as_bytes(), budget, cancelled)?;
        let candidate = image.prepare_retained_index_drop(
            self.table(),
            self.index_name(),
            budget,
            cancelled,
        )?;
        let NativeIndexDropParts {
            effect,
            table,
            index,
        } = candidate.into_parts();
        if effect.is_unique_drop() || Identity::from_effect(&effect) != self.data.identity {
            return Err(IndexDropIntentError::IdentityMismatch);
        }
        // The borrowed native API owns its paid request names. Move those exact
        // buffers into the retained original while preserving its bound fields.
        Ok((
            effect,
            Self {
                data: OriginalData {
                    table,
                    index,
                    ..self.data
                },
            },
        ))
    }
}
