//! Numeric index originals bind scalar identity and all generated witnesses.
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
mod recovery;
mod scalar;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableNativeNumericIndexCreatePlan;
pub use recovery::QualifiedNativeNumericIndexCreateRecovery;

use super::{NativeIntentError, binding};
use crate::{EditError, InspectError, NativeRecoveryQualificationError, header::check};
#[cfg(feature = "ace16-create")]
use crate::{
    NativeImage, SecondaryIndexCreate,
    table::mutation::normal_numeric_index_creation::effect::{
        NativeNumericIndexCreateEffect, NativeNumericIndexCreateParts,
    },
};
use scalar::NumericType;
use tablecraft_core::limits::{LimitError, ResourceBudget};
use tablecraft_schema::{ScalarType, SchemaName};
use thiserror::Error;

/// Original index framing, whole-source binding or fresh native reconstruction failed.
#[derive(Debug, Error)]
pub enum NumericIndexCreateIntentError {
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

impl From<LimitError> for NumericIndexCreateIntentError {
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
    logical: u32,
    physical: u32,
    numeric: NumericType,
}

/// TCIC0003 numeric kind and target/allocation witnesses bound to all BEFORE bytes.
/// Scalar tags describe native field types, not Rust enum discriminants or values
/// inferred from selected rows. The numeric constructor repeats its full source
/// admission and independently derives the same logical/physical identities.
/// Decoded data grants no source certificate, physical page scope or write authority.
pub struct NativeNumericIndexCreateIntent {
    table: String,
    column: String,
    index: SchemaName,
    descending: bool,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; binding::DIGEST_BYTES],
}

impl NativeNumericIndexCreateIntent {
    /// Fixed framing plus three native names of at most 64 UTF-16 units each.
    /// This UTF-8 envelope does not replace native identifier validation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;

    /// Decode only TCIC0003/profile3. No old mutation family is attempted.
    ///
    /// # Errors
    /// Rejects invalid framing, profile, flags, identity, names, allocation,
    /// cancellation and caller resource exhaustion before publishing an owner.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NumericIndexCreateIntentError> {
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
    ) -> Result<Vec<u8>, NumericIndexCreateIntentError> {
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
        self.index.as_str()
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

    /// Generated logical identity, independently rederived from the complete source.
    pub const fn logical_index_number(&self) -> u32 {
        self.identity.logical
    }

    /// Generated dense physical array identity, separate from logical history.
    pub const fn physical_index_number(&self) -> u32 {
        self.identity.physical
    }

    /// Original native scalar type tag, independent from Rust enum layout.
    pub const fn native_type(&self) -> u8 {
        self.identity.numeric.native_type()
    }

    /// The exact qualified scalar carrier associated with the native type tag.
    pub const fn scalar_type(&self) -> ScalarType {
        self.identity.numeric.scalar_type()
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
    ) -> Result<(), NumericIndexCreateIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(NumericIndexCreateIntentError::BeforeMismatch);
        }
        if binding::digest_retained(before, budget, cancelled)? != self.before_binding {
            return Err(NumericIndexCreateIntentError::BeforeMismatch);
        }
        Ok(())
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        parts: NativeNumericIndexCreateParts,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeNumericIndexCreateEffect, Self), NumericIndexCreateIntentError> {
        let NativeNumericIndexCreateParts {
            effect,
            table,
            column,
            index,
        } = parts;
        let identity = Identity::from_effect(&effect)?;
        let before_length = effect.before_bytes().len() as u64;
        codec::validate(
            identity,
            before_length,
            [&table, &column, index.as_str()],
            budget,
            cancelled,
        )?;
        let before_binding = binding::digest_retained(effect.before_bytes(), budget, cancelled)?;
        let intent = Self {
            table,
            column,
            index,
            descending: effect.descending(),
            identity,
            before_length,
            before_binding,
        };
        Ok((effect, intent))
    }

    #[cfg(feature = "ace16-create")]
    fn reconstruct(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeNumericIndexCreateEffect, Self), NumericIndexCreateIntentError> {
        self.verify_before(image.as_bytes(), budget, cancelled)?;
        let candidate = image.prepare_retained_numeric_index_creation(
            self.table(),
            &SecondaryIndexCreate::new(self.index_name(), self.column(), self.descending()),
            budget,
            cancelled,
        )?;
        let NativeNumericIndexCreateParts {
            effect,
            table,
            column,
            index,
        } = candidate.into_numeric_parts();
        if Identity::from_effect(&effect)? != self.identity {
            return Err(NumericIndexCreateIntentError::IdentityMismatch);
        }
        // The borrowed public native constructor returns its own paid request
        // buffers. Move those validated names into the resulting original while
        // retaining the original binding and identity fields; no buffer is cloned.
        Ok((
            effect,
            Self {
                table,
                column,
                index,
                ..self
            },
        ))
    }
}

#[cfg(feature = "ace16-create")]
impl Identity {
    fn from_effect(
        effect: &NativeNumericIndexCreateEffect,
    ) -> Result<Self, NumericIndexCreateIntentError> {
        Ok(Self {
            native_table: effect.native_table_id(),
            definition: effect.definition_page(),
            number: effect
                .native_number()
                .try_into()
                .map_err(|_| NumericIndexCreateIntentError::InvalidRecord)?,
            column_id: effect.column_id(),
            logical: effect.logical_index_number(),
            physical: effect.physical_index_number(),
            numeric: NumericType::from_scalar(effect.scalar_type())?,
        })
    }
}

#[cfg(test)]
mod tests;
