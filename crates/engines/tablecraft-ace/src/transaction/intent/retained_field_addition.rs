//! A single typed ADD original is separate from every older column language.
#[cfg(feature = "ace16-create")]
mod codec;
#[cfg(feature = "ace16-create")]
mod planning;
#[cfg(feature = "ace16-create")]
mod recovery;

#[cfg(feature = "ace16-create")]
pub use planning::RecoverableRetainedTypedFieldAdditionPlan;
#[cfg(feature = "ace16-create")]
pub use recovery::QualifiedRetainedTypedFieldAdditionRecovery;

use crate::{CurrentFieldAdditionError, InspectError, NativeRecoveryQualificationError};
#[cfg(feature = "ace16-create")]
use crate::{
    RetainedFieldAdditionDefinition,
    header::check,
    transaction::intent::{NativeIntentError, binding},
};
#[cfg(feature = "ace16-create")]
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Typed original framing, declaration, source or independent image failure.
#[derive(Debug, Error)]
pub enum RetainedTypedFieldAdditionIntentError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    Declaration(#[from] crate::CreateDatabaseError),
    #[error(transparent)]
    Field(#[from] CurrentFieldAdditionError),
    #[error(transparent)]
    Proof(#[from] NativeRecoveryQualificationError),
    #[error("invalid retained typed field-addition original")]
    InvalidRecord,
    #[error("retained typed field-addition original image differs")]
    BeforeMismatch,
    #[error("retained typed field addition requires native creation support")]
    UnsupportedSource,
}

/// Exact owned single-column declaration and complete original-image binding.
/// This request data cannot supply a source certificate or native edit scope.
#[cfg(feature = "ace16-create")]
pub struct NativeRetainedTypedFieldAdditionIntent {
    table: String,
    definition: RetainedFieldAdditionDefinition,
    before_length: u64,
    before_binding: [u8; 32],
}

#[cfg(feature = "ace16-create")]
impl NativeRetainedTypedFieldAdditionIntent {
    /// Native 64-unit names, fixed framing and the largest exact default carrier.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_BYTES;

    pub fn new(
        table: &str,
        definition: RetainedFieldAdditionDefinition,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RetainedTypedFieldAdditionIntentError> {
        codec::validate_name(table, budget, cancelled)?;
        let before_binding = binding::digest_retained(before, budget, cancelled)?;
        let table = codec::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            definition,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    pub fn table(&self) -> &str {
        &self.table
    }
    pub fn column(&self) -> &str {
        self.definition.name()
    }
    pub fn definition(&self) -> &RetainedFieldAdditionDefinition {
        &self.definition
    }

    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, RetainedTypedFieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode exactly TCAC0012/profile12, without trying any older declaration.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, RetainedTypedFieldAdditionIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), RetainedTypedFieldAdditionIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest_retained(before, budget, cancelled)? != self.before_binding
        {
            return Err(RetainedTypedFieldAdditionIntentError::BeforeMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
