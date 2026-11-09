//! Closed explicit-zero LONG declarations, separate from frozen TCAC0001/2.
mod codec;
mod planning;
mod recovery;

use super::{NativeIntentError, binding};
use crate::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError,
    CreatedDefaultZeroColumnRequired, InspectError, NativeImage, header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub use planning::RecoverableDefaultZeroAddColumnPlan;
pub use recovery::{
    QualifiedDefaultZeroAddColumnRecovery, qualify_native_default_zero_add_column_recovery,
};

const BINDING_BYTES: usize = 32;
const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Failure before an explicit declaration gains persistence/recovery authority.
#[derive(Debug, Error)]
pub enum DefaultZeroAddColumnIntentError {
    #[error(transparent)]
    /// Native work, allocation, cancellation or inspection failed.
    Inspect(#[from] InspectError),
    #[error(transparent)]
    /// Complete original source binding failed.
    Binding(#[from] NativeIntentError),
    #[error(transparent)]
    /// The actual source, declaration or property/allocation proof refused.
    Column(#[from] CreatedDefaultZeroAddColumnError),
    #[error("invalid explicit-zero add-column intent record")]
    /// Unknown language, declaration tag, framing or borrowed name.
    InvalidRecord,
    #[error("explicit-zero add-column before image does not match")]
    /// Captured source length or digest differs.
    BeforeMismatch,
    #[error(transparent)]
    /// Complete image proof exhausted or failed outside a direct mismatch.
    Proof(#[from] crate::NativeRecoveryQualificationError),
    #[error("explicit-zero add-column reconstruction differs")]
    /// Fresh complete native reconstruction differs from the owning candidate.
    ImageMismatch,
}

impl From<LimitError> for DefaultZeroAddColumnIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// TCAC0003/profile3 data only: nullable LONG, DefaultValue Int32 zero, and exact
/// Required-property presence. Decoding cannot construct a normal write plan.
/// The absent and explicitly stored false declarations are never normalized.
pub struct NativeDefaultZeroAddColumnIntent {
    table: SchemaName,
    column: SchemaName,
    required: CreatedDefaultZeroColumnRequired,
    before_length: u64,
    before_binding: [u8; BINDING_BYTES],
}

impl NativeDefaultZeroAddColumnIntent {
    /// Bound checked before a consumer retains any request buffer.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    fn capture(
        candidate: &CreatedDefaultZeroAddColumnCandidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DefaultZeroAddColumnIntentError> {
        Ok(Self {
            table: copy_name(candidate.table(), budget, cancelled)?,
            column: copy_name(candidate.column(), budget, cancelled)?,
            required: candidate.required_property(),
            before_length: candidate.before_bytes().len() as u64,
            before_binding: binding::digest(candidate.before_bytes(), budget, cancelled)?,
        })
    }

    fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CreatedDefaultZeroAddColumnCandidate, DefaultZeroAddColumnIntentError> {
        Ok(
            image.prepare_created_add_default_zero_int32_column_candidate(
                self.table(),
                self.column(),
                self.required,
                budget,
                cancelled,
            )?,
        )
    }

    /// Exact source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact requested field spelling.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Explicit Required-property declaration; no nullability inference.
    pub fn required_property(&self) -> CreatedDefaultZeroColumnRequired {
        self.required
    }

    /// Encode the original declaration and source binding, never an AFTER patch.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, DefaultZeroAddColumnIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Reject every other TCAC version/declaration before either name allocation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DefaultZeroAddColumnIntentError> {
        codec::decode(bytes, budget, cancelled)
    }

    /// Check the complete original digest/length, without granting write authority.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), DefaultZeroAddColumnIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length
            || binding::digest(bytes, budget, cancelled)? != self.before_binding
        {
            return Err(DefaultZeroAddColumnIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), DefaultZeroAddColumnIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(DefaultZeroAddColumnIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(DefaultZeroAddColumnIntentError::InvalidRecord);
    }
    Ok(())
}

fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, DefaultZeroAddColumnIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| DefaultZeroAddColumnIntentError::InvalidRecord)
}

#[cfg(test)]
mod tests;
