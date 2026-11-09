//! Fitted ADD and chronological source replay after explicit Name AutoCorrect OFF.
pub(super) mod availability;
mod history;
mod history_candidate;
mod kernel;
mod source;
#[cfg(test)]
mod tests;
mod verify;

use super::{FieldAdditionError, FieldAdditionTarget};
use crate::{
    EditError, FieldHistoryDatabaseOptionsError, InspectError, NativeImage,
    creation::FieldHistoryOffSource, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

pub use history::FieldAdditionAfterOffHistoryError;
pub use history_candidate::FieldAdditionAfterOffHistoryCandidate;

const TEXT_UNITS: usize = 255;
pub(super) const MAX_ASCII_NAME_BYTES: usize = 64;

/// Refusal from the separate fitting ADD-after-explicit-OFF candidate profile.
#[derive(Debug, Error)]
pub enum FieldAdditionAfterOffError {
    /// Native read, cancellation, allocation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The actual settled source failed its full inverse/constructive replay.
    #[error(transparent)]
    SourceProof(#[from] FieldHistoryDatabaseOptionsError),
    /// Existing typed declaration, canonical definition or property construction failed.
    #[error(transparent)]
    Field(#[from] FieldAdditionError),
    /// Actual selected-root or owned property placement refused.
    #[error(transparent)]
    Placement(#[from] EditError),
    /// The requested declaration is outside the independently observed target tuples.
    #[error("field declaration is outside ADD-after-OFF candidate scope")]
    Target,
    /// Requested or compared native names are outside the qualified ASCII namespace.
    #[error("ADD-after-OFF requires an ASCII comparison namespace")]
    Namespace,
    /// Complete original identity, properties or byte complement changed unexpectedly.
    #[error("ADD-after-OFF preservation check failed")]
    Preservation,
}

impl From<tablecraft_core::limits::LimitError> for FieldAdditionAfterOffError {
    fn from(error: tablecraft_core::limits::LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// A source-bound native candidate, separate from every TCAC1–5 save/intent type.
/// There is no constructor from a source-check boolean or arbitrary AFTER bytes.
pub struct FieldAdditionAfterOffCandidate {
    proof: FieldHistoryOffSource,
    after: Vec<u8>,
    table: String,
    column: String,
    target: FieldAdditionTarget,
    columns: usize,
}

impl FieldAdditionAfterOffCandidate {
    /// Complete original retained by the actual settled-source qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.proof.image().as_bytes()
    }

    /// Complete checked fitting candidate, with no persistence authority.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Exact existing table spelling from the qualified source.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact proposed field spelling, without normalization.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Requested closed declaration; no default or compression policy is inferred.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.target
    }

    /// Complete resulting field count, not a property-row count.
    pub const fn column_count(&self) -> usize {
        self.columns
    }
}

impl NativeImage {
    /// Append Number or compressed TEXT(255) after a proven explicit OFF insertion.
    ///
    /// The input retains the settled checker's original nine-role, eleven-page,
    /// empty generated-primary source language. The database body/ROW and every
    /// old property body remain exact. The new user body fits after the database
    /// row. Requested and compared table/field names must be ASCII; Unicode
    /// collision semantics are not qualified by this profile. Allocation, rollover,
    /// later-event source admission and normal saves
    /// remain outside this separate unpublished candidate.
    pub fn prepare_created_add_field_after_off_candidate(
        self,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionAfterOffCandidate, FieldAdditionAfterOffError> {
        let _span = tracing::debug_span!("native.field_addition.after_explicit_off").entered();
        check_request(table, column, target, budget, cancelled)?;
        let source = source::Source::capture(self, table, budget, cancelled)?;
        let built = kernel::append(source, column, target, budget, cancelled)?;
        Ok(FieldAdditionAfterOffCandidate {
            proof: built.proof,
            after: built.after,
            table: built.table,
            column: built.column,
            target,
            columns: built.columns,
        })
    }
}

fn check_request(
    table: &str,
    column: &str,
    target: FieldAdditionTarget,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldAdditionAfterOffError> {
    check(budget, cancelled)?;
    if table.len() > MAX_ASCII_NAME_BYTES || column.len() > MAX_ASCII_NAME_BYTES {
        return Err(FieldAdditionAfterOffError::Namespace);
    }
    budget.charge(
        tablecraft_core::limits::Resource::Steps,
        (table.len() + column.len()) as u64,
    )?;
    if !table.is_ascii() || !column.is_ascii() {
        return Err(FieldAdditionAfterOffError::Namespace);
    }
    if !matches!(
        target,
        FieldAdditionTarget::NumberLongZero
            | FieldAdditionTarget::ShortText {
                max_utf16_units: TEXT_UNITS,
                unicode_compression: true
            }
    ) {
        return Err(FieldAdditionAfterOffError::Target);
    }
    crate::creation::request::name(column, budget, cancelled).map_err(FieldAdditionError::from)?;
    Ok(())
}
