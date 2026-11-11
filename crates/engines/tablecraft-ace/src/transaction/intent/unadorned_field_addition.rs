//! SQL declaration originals preserve absent policies independently of gallery ADD.
#[cfg(feature = "ace16-create")]
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
#[cfg(feature = "ace16-create")]
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableUnadornedFieldAdditionPlan;
#[cfg(feature = "ace16-create")]
pub use recovery::QualifiedUnadornedFieldAdditionRecovery;

use crate::{CurrentFieldAdditionError, InspectError, NativeRecoveryQualificationError};
#[cfg(feature = "ace16-create")]
use crate::{
    NativeImage, NativeUnadornedFieldAdditionCandidate, UnadornedFieldAdditionDefinition,
    header::check,
    table::{UnadornedFieldAdditionEffect, UnadornedFieldAdditionSourceProfile},
    transaction::intent::{NativeIntentError, binding},
};
#[cfg(feature = "ace16-create")]
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Exact declared ADD framing, original binding or native proof failed.
#[derive(Debug, Error)]
pub enum UnadornedFieldAdditionIntentError {
    /// Inspection, allocation, cumulative resource control or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The original request binding or native image geometry is invalid.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The column is outside the closed unadorned scalar declaration language.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Declaration(#[from] crate::CreateDatabaseError),
    /// Actual source, target placement, dependencies or independent proof refused.
    #[error(transparent)]
    Field(#[from] CurrentFieldAdditionError),
    /// Complete image comparison or fresh reconstruction failed.
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    /// Wrong family, framing, reserved bits or identity representation.
    #[error("invalid unadorned field-addition original")]
    InvalidRecord,
    /// The complete original image length or digest differs.
    #[error("unadorned field-addition original image differs")]
    BeforeMismatch,
    /// Fresh native source or allocated field identity differs from the original.
    #[error("unadorned field-addition identity differs")]
    IdentityMismatch,
    /// Native creation/source reconstruction is unavailable in this feature build.
    #[error("unadorned field addition requires native creation support")]
    UnsupportedSource,
}

#[cfg(feature = "ace16-create")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceClass {
    Keyless,
    Indexed,
}

#[cfg(feature = "ace16-create")]
impl SourceClass {
    const fn tag(self) -> u8 {
        match self {
            Self::Keyless => 1,
            Self::Indexed => 2,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, UnadornedFieldAdditionIntentError> {
        match tag {
            1 => Ok(Self::Keyless),
            2 => Ok(Self::Indexed),
            _ => Err(UnadornedFieldAdditionIntentError::InvalidRecord),
        }
    }
}

#[cfg(feature = "ace16-create")]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    source: SourceClass,
    native_table: i32,
    definition: u32,
    native_number: u16,
    column_id: u16,
    column_count: u16,
}

/// Exact unadorned declaration, original-image binding and native identity facts.
/// Decoding data cannot supply a native source certificate or persistence scope.
#[cfg(feature = "ace16-create")]
pub struct NativeUnadornedFieldAdditionIntent {
    table: String,
    definition: UnadornedFieldAdditionDefinition,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; 32],
}

#[cfg(feature = "ace16-create")]
impl NativeUnadornedFieldAdditionIntent {
    /// Fixed TCAC14 framing plus the actual native identifier representation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;

    /// Exact source-resolved original table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }
    /// Exact requested field name.
    pub fn column(&self) -> &str {
        self.definition.name()
    }
    /// Original declaration with Required, AZL, default and description absent.
    pub fn definition(&self) -> &UnadornedFieldAdditionDefinition {
        &self.definition
    }
    /// Original signed native catalog identity, not a caller-issued object scope.
    pub const fn native_table_id(&self) -> i32 {
        self.identity.native_table
    }
    /// Original physical table-definition root.
    pub const fn definition_page(&self) -> u32 {
        self.identity.definition
    }
    /// Allocated native field ordinal, separate from display position.
    pub const fn native_number(&self) -> u16 {
        self.identity.native_number
    }
    /// Allocated stored creation-time field identity.
    pub const fn column_id(&self) -> u16 {
        self.identity.column_id
    }
    /// Replacement active-field count, separate from retained high-water counts.
    pub const fn column_count(&self) -> u16 {
        self.identity.column_count
    }

    /// Encode only the closed TCAC0014 original declaration and whole binding.
    ///
    /// # Errors
    /// Refuses invalid names/identities and retains allocation, budget and cancellation errors.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, UnadornedFieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode exactly TCAC0014/profile14; no older decoder is attempted.
    ///
    /// # Errors
    /// Refuses framing, other policies/types, invalid native geometry or identities,
    /// allocation failure, cumulative resource exhaustion and cancellation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, UnadornedFieldAdditionIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Verify the entire original source before any native reconstruction authority.
    ///
    /// # Errors
    /// Returns BeforeMismatch for changed bytes, or typed geometry/budget/cancellation failures.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), UnadornedFieldAdditionIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(UnadornedFieldAdditionIntentError::BeforeMismatch);
        }
        let actual = binding::digest_retained(before, budget, cancelled)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(UnadornedFieldAdditionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn from_candidate(
        candidate: NativeUnadornedFieldAdditionCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(UnadornedFieldAdditionEffect, Self), UnadornedFieldAdditionIntentError> {
        let parts = candidate.into_parts(budget, cancelled)?;
        let effect = parts.effect;
        let identity = Identity::from_effect(&effect)?;
        let before_length = effect.before_bytes().len() as u64;
        codec::validate_identity(identity, before_length)?;
        let before_binding = binding::digest_retained(effect.before_bytes(), budget, cancelled)?;
        // The durable original owns its table spelling independently of the native
        // source/schema proof. The actual declaration itself moves without copying.
        let table = codec::copy_table(effect.table(), budget, cancelled)?;
        Ok((
            effect,
            Self {
                table,
                definition: parts.definition,
                identity,
                before_length,
                before_binding,
            },
        ))
    }

    /// Caller already checked this binding against these exact captured image bytes.
    fn rebuild_verified(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(UnadornedFieldAdditionEffect, Self), UnadornedFieldAdditionIntentError> {
        let Self {
            table,
            definition,
            identity,
            before_length,
            before_binding,
        } = self;
        let candidate = image
            .prepare_retained_unadorned_field_addition(&table, definition, budget, cancelled)?;
        let parts = candidate.into_parts(budget, cancelled)?;
        if Identity::from_effect(&parts.effect)? != identity {
            return Err(UnadornedFieldAdditionIntentError::IdentityMismatch);
        }
        check(budget, cancelled)?;
        Ok((
            parts.effect,
            Self {
                table,
                definition: parts.definition,
                identity,
                before_length,
                before_binding,
            },
        ))
    }
}

#[cfg(feature = "ace16-create")]
impl Identity {
    fn from_effect(
        effect: &UnadornedFieldAdditionEffect,
    ) -> Result<Self, UnadornedFieldAdditionIntentError> {
        Ok(Self {
            source: match effect.source_profile() {
                UnadornedFieldAdditionSourceProfile::Keyless => SourceClass::Keyless,
                UnadornedFieldAdditionSourceProfile::Indexed => SourceClass::Indexed,
            },
            native_table: effect.native_table_id(),
            definition: effect.definition_page(),
            native_number: u16::try_from(effect.native_number())
                .map_err(|_| UnadornedFieldAdditionIntentError::IdentityMismatch)?,
            column_id: effect.column_id(),
            column_count: u16::try_from(effect.column_count())
                .map_err(|_| UnadornedFieldAdditionIntentError::IdentityMismatch)?,
        })
    }
}

#[cfg(all(test, feature = "ace16-create"))]
mod tests;
