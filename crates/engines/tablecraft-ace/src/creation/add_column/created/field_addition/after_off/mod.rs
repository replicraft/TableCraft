//! Separate unpublished ADD over one completely owned settled explicit-OFF source.
mod source;
#[cfg(test)]
mod tests;
mod verify;

use super::{FieldAdditionError, FieldAdditionTarget, build};
use crate::{
    CreateColumn, EditError, FieldHistoryDatabaseOptionsError, InspectError, NativeImage,
    creation::{
        FieldHistoryOffSource,
        add_column::{common, property_preservation},
    },
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

const TEXT_UNITS: usize = 255;

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
        check(budget, cancelled)?;
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
        crate::creation::request::name(column, budget, cancelled)
            .map_err(FieldAdditionError::from)?;
        let source = source::Source::capture(self, table, budget, cancelled)?;
        let mut columns = source
            .definition
            .borrowed_columns(budget, cancelled)
            .map_err(FieldAdditionError::from)?;
        budget.charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)?;
        columns
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        columns.push(target.declaration(column)?);
        let order = build::extend_order(source.order, &columns, target, budget, cancelled)?;
        let image = source.proof.image();
        let rebuilt = build::canonical(
            source.checked.schema().name(),
            &columns,
            order,
            budget,
            cancelled,
        )?;
        let mut after = common::copy_definition(
            image,
            &rebuilt,
            source.checked.schema().definition_page() as usize * PAGE_BYTES,
            budget,
            cancelled,
        )?;
        super::verify::definition(
            image,
            &after,
            &source.checked,
            column,
            target,
            budget,
            cancelled,
        )?;
        let body = build::property_body(&columns, order, budget, cancelled)?;
        let change = catalog_property::compose_fitted_at_origin(
            image,
            source.checked.schema(),
            body,
            budget,
            cancelled,
        )?;
        change.apply(&mut after, budget, cancelled)?;
        property_preservation::repeated(
            image,
            &after,
            &source.checked,
            column,
            &change,
            budget,
            cancelled,
        )
        .map_err(FieldAdditionError::from)?;
        verify::candidate(&source, &after, &change, budget, cancelled)?;
        let table = super::super::super::owned(source.checked.schema().name(), budget)?;
        let column = super::super::super::owned(column, budget)?;
        let count = columns.len();
        drop(columns);
        check(budget, cancelled)?;
        Ok(FieldAdditionAfterOffCandidate {
            proof: source.proof,
            after,
            table,
            column,
            target,
            columns: count,
        })
    }
}
