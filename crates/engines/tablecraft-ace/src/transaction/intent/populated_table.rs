//! Original populated-table declarations and values, never SQL or save authority.
mod codec;
mod recovery;
mod write;

use super::{FirstTableIntentError, NativeIntentError, binding};
use crate::{FirstTableDefinition, InspectError, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use thiserror::Error;

pub use recovery::QualifiedPopulatedAddTableRecovery;
pub use write::PreparedPopulatedAddTable;

/// Original materialized-request framing and source-binding failure.
#[derive(Debug, Error)]
pub enum PopulatedTableIntentError {
    /// Caller work, cancellation or allocation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Shared scalar framing or complete source digest failure.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The nested native declaration template failed closed validation.
    #[error(transparent)]
    Declaration(#[from] FirstTableIntentError),
    /// Malformed length, count, carrier, flags, reserved bytes or value declaration.
    #[error("invalid populated-table intent record")]
    InvalidRecord,
    /// Only the exact TCPT0001 language is supported.
    #[error("populated-table intent version is unsupported")]
    UnsupportedVersion,
    /// Only unadorned native manual declarations belong to this profile.
    #[error("populated-table construction profile is unsupported")]
    UnsupportedProfile,
    /// The entire original image differs from the retained source binding.
    #[error("populated-table intent before image does not match")]
    BeforeMismatch,
}

/// Closed TCPT0001 request data: manual native declarations, every resolved cell,
/// and the actual full BEFORE binding. The nested TCNT7 canonical-blank envelope
/// is an inert declaration codec; it never identifies this operation's source.
/// Decoding or constructing this data grants no native write/recovery permission.
pub struct NativePopulatedAddTableIntent {
    definition: FirstTableDefinition,
    rows: Vec<Vec<Value>>,
    before_length: u64,
    before_binding: [u8; 32],
}

impl NativePopulatedAddTableIntent {
    /// The complete wire is u64-length framed and must fit this platform's Vec.
    /// Caller allowances and fallible allocation remain independent; this is not
    /// a promise that any such allocation or native output is admissible.
    pub const MAX_ENCODED_BYTES: usize = usize::MAX;

    /// Bind complete original values to actual source bytes without planning,
    /// running a query, opening a file or inferring source mutation eligibility.
    pub fn new(
        definition: FirstTableDefinition,
        rows: Vec<Vec<Value>>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PopulatedTableIntentError> {
        codec::validate_request(&definition, &rows, budget, cancelled)?;
        let before_binding = binding::digest_retained(before, budget, cancelled)?;
        Ok(Self {
            definition,
            rows,
            before_length: before.len() as u64,
            before_binding,
        })
    }

    /// Requested local destination, never an existing replacement or SQL string.
    pub fn table(&self) -> &str {
        self.definition.table()
    }
    /// Complete original row count, including zero, without regenerated defaults.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    /// Exact explicit declaration count; no key or generation field is invented.
    pub fn column_count(&self) -> usize {
        self.definition.column_count()
    }

    /// Serialize resolved declarations/values and full source binding only.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, PopulatedTableIntentError> {
        codec::encode(
            &self.definition,
            &self.rows,
            self.before_length,
            self.before_binding,
            budget,
            cancelled,
        )
    }

    /// Decode only TCPT0001 after full envelope/cell preflight, without another
    /// action decoder, default application, SQL evaluation or native planning.
    pub fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PopulatedTableIntentError> {
        codec::decode(raw, budget, cancelled)
    }

    fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), PopulatedTableIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length
            || binding::digest_retained(before, budget, cancelled)? != self.before_binding
        {
            return Err(PopulatedTableIntentError::BeforeMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
