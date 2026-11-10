//! Explicit second source profile; no widening or retry of TCAC0001/profile 1.
mod default_zero;
mod field_addition;
pub use default_zero::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError,
    CreatedDefaultZeroColumnRequired, RepeatedDefaultZeroAddColumnCandidate,
};
pub use field_addition::{
    CreatedFieldAdditionSourcePurpose, FieldAdditionAfterOffCandidate, FieldAdditionAfterOffError,
    FieldAdditionAfterOffHistoryCandidate, FieldAdditionAfterOffHistoryError,
    FieldAdditionCandidate, FieldAdditionError, FieldAdditionSourceAvailability,
    FieldAdditionSourceError, FieldAdditionSourcePurpose,
};
mod preservation;
mod source;

use super::{AddColumnCandidate, common, owned};
use crate::{
    CreateColumn, CreateDatabaseError, FirstTableDefinition, InspectError, NativeImage,
    create_ace16_database, format::page::PAGE_BYTES, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;
use thiserror::Error;

/// Refusal for canonical generated-primary source profile 2 candidates.
#[derive(Debug, Error)]
pub enum CreatedAddColumnError {
    /// Native inspection, resources, cancellation or allocation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The proposed declaration is outside constructive schema limits.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// The actual complete source is outside this separate constructive profile.
    #[error("database is outside generated-primary add-column source profile 2")]
    Source,
    /// A candidate changed bytes or declarations outside its checked envelope.
    #[error("generated-primary add-column preservation check failed")]
    Preservation,
}

/// Profile-2 candidate with no journal, recovery or normal-save conversion.
/// The original TCAC0001/profile-1 API never returns or accepts this type.
pub struct CreatedAddColumnCandidate {
    image: AddColumnCandidate,
    definition: FirstTableDefinition,
}

impl NativeImage {
    /// Append an ordinary nullable Int32 field with an absent DefaultValue.
    ///
    /// The actual header-5 source must equal the constructive single empty table
    /// with a first-field LONG Increment primary, one through fifteen supported
    /// scalar declarations, and only constructively reproducible inline field
    /// Required/AllowZeroLength/constant-default/Description properties. Complete
    /// source equality closes allocation, program/catalog roles and unknown tails.
    /// Nonzero timestamps, additional objects, external property LVAL storage,
    /// populated tables and broader keys refuse. Existing defaults are preserved;
    /// no new default, generated value or UI permission is inferred.
    pub fn prepare_created_add_int32_column_candidate(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CreatedAddColumnCandidate, CreatedAddColumnError> {
        let _span = tracing::debug_span!("native.created_add_column.prepare_candidate").entered();
        check(budget, cancelled)?;
        super::super::request::name(column, budget, cancelled)?;
        let (checked, definition) = source::capture(&self, table, budget, cancelled)?;
        let schema = checked.schema();
        let mut columns = definition.borrowed_columns(budget, cancelled)?;
        // The borrowed declaration vector must account for its appended slot.
        budget
            .charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)
            .map_err(InspectError::from)?;
        columns
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        columns.push(CreateColumn::new(column, ScalarType::Int32));
        let rebuilt = create_ace16_database(schema.name(), &columns, &[], budget, cancelled)?;
        let after = common::copy_definition(
            &self,
            rebuilt.as_bytes(),
            schema.definition_page() as usize * PAGE_BYTES,
            budget,
            cancelled,
        )?;
        preservation::verify(&self, &after, &checked, column, budget, cancelled)?;
        let table = owned(schema.name(), budget)?;
        let column = owned(column, budget)?;
        let count = columns.len();
        drop(columns);
        check(budget, cancelled)?;
        Ok(CreatedAddColumnCandidate {
            image: AddColumnCandidate {
                before: self,
                after,
                table,
                column,
                columns: count,
            },
            definition,
        })
    }
}

impl CreatedAddColumnCandidate {
    /// Complete unchanged original source.
    pub fn before_bytes(&self) -> &[u8] {
        self.image.before_bytes()
    }

    /// Complete proposed image, without normal-save authority.
    pub fn after_bytes(&self) -> &[u8] {
        self.image.after_bytes()
    }

    /// Original decoded table spelling.
    pub fn table(&self) -> &str {
        self.image.table()
    }

    /// Exact proposed field spelling.
    pub fn column(&self) -> &str {
        self.image.column()
    }

    /// Resulting declaration count.
    pub const fn column_count(&self) -> usize {
        self.image.column_count()
    }

    /// Explicit candidate source profile, independent of any journal wire profile.
    pub const fn source_profile(&self) -> u8 {
        2
    }

    /// Original reconstructed declarations, usable for future frozen request proof.
    pub fn source_definition(&self) -> &FirstTableDefinition {
        &self.definition
    }
}

#[cfg(test)]
mod tests;
