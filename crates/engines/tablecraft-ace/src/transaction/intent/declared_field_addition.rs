//! SQL declaration originals preserve absent policies independently of gallery ADD.
#[cfg(feature = "ace16-create")]
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
#[cfg(feature = "ace16-create")]
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableDeclaredFieldAdditionPlan;
#[cfg(feature = "ace16-create")]
pub use recovery::QualifiedDeclaredFieldAdditionRecovery;

use crate::{CurrentFieldAdditionError, InspectError, NativeRecoveryQualificationError};
#[cfg(feature = "ace16-create")]
use crate::{
    DeclaredFieldAdditionDefinition, NativeDeclaredFieldAdditionCandidate, NativeImage,
    header::check,
    table::DeclaredFieldAdditionEffect,
    transaction::intent::{NativeIntentError, binding},
};
#[cfg(feature = "ace16-create")]
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Exact declared ADD framing, original binding or native proof failed.
#[derive(Debug, Error)]
pub enum DeclaredFieldAdditionIntentError {
    /// Inspection, allocation, cumulative resource control or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The original request binding or native image geometry is invalid.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The column is outside the closed unadorned Currency declaration language.
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
    #[error("invalid declared field-addition original")]
    InvalidRecord,
    /// The complete original image length or digest differs.
    #[error("declared field-addition original image differs")]
    BeforeMismatch,
    /// Fresh native source or allocated field identity differs from the original.
    #[error("declared field-addition identity differs")]
    IdentityMismatch,
    /// Native creation/source reconstruction is unavailable in this feature build.
    #[error("declared field addition requires native creation support")]
    UnsupportedSource,
}

#[cfg(feature = "ace16-create")]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    native_table: i32,
    definition: u32,
    native_number: u16,
    column_id: u16,
    column_count: u16,
}

/// Exact unadorned declaration, original-image binding and native identity facts.
/// Decoding data cannot supply a native source certificate or persistence scope.
#[cfg(feature = "ace16-create")]
pub struct NativeDeclaredFieldAdditionIntent {
    table: String,
    definition: DeclaredFieldAdditionDefinition,
    identity: Identity,
    before_length: u64,
    before_binding: [u8; 32],
}

#[cfg(feature = "ace16-create")]
impl NativeDeclaredFieldAdditionIntent {
    /// Fixed TCAC13 framing plus the actual native identifier representation.
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
    pub fn definition(&self) -> &DeclaredFieldAdditionDefinition {
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

    /// Encode only the closed TCAC0013 original declaration and whole binding.
    ///
    /// # Errors
    /// Refuses invalid names/identities and retains allocation, budget and cancellation errors.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, DeclaredFieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode exactly TCAC0013/profile13; no older decoder is attempted.
    ///
    /// # Errors
    /// Refuses framing, other policies/types, invalid native geometry or identities,
    /// allocation failure, cumulative resource exhaustion and cancellation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DeclaredFieldAdditionIntentError> {
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
    ) -> Result<(), DeclaredFieldAdditionIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(DeclaredFieldAdditionIntentError::BeforeMismatch);
        }
        let actual = binding::digest_retained(before, budget, cancelled)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(DeclaredFieldAdditionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    fn from_candidate(
        candidate: NativeDeclaredFieldAdditionCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(DeclaredFieldAdditionEffect, Self), DeclaredFieldAdditionIntentError> {
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
    ) -> Result<(DeclaredFieldAdditionEffect, Self), DeclaredFieldAdditionIntentError> {
        let Self {
            table,
            definition,
            identity,
            before_length,
            before_binding,
        } = self;
        let candidate = image
            .prepare_retained_declared_field_addition(&table, definition, budget, cancelled)?;
        let parts = candidate.into_parts(budget, cancelled)?;
        if Identity::from_effect(&parts.effect)? != identity {
            return Err(DeclaredFieldAdditionIntentError::IdentityMismatch);
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
        effect: &DeclaredFieldAdditionEffect,
    ) -> Result<Self, DeclaredFieldAdditionIntentError> {
        Ok(Self {
            native_table: effect.native_table_id(),
            definition: effect.definition_page(),
            native_number: u16::try_from(effect.native_number())
                .map_err(|_| DeclaredFieldAdditionIntentError::IdentityMismatch)?,
            column_id: effect.column_id(),
            column_count: u16::try_from(effect.column_count())
                .map_err(|_| DeclaredFieldAdditionIntentError::IdentityMismatch)?,
        })
    }
}

#[cfg(all(test, feature = "ace16-create"))]
mod tests;
