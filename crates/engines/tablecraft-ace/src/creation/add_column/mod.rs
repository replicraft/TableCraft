//! Source-qualified empty-table column candidates, without normal save authority.
mod build;
mod common;
#[cfg(feature = "journal")]
mod created;
#[cfg(feature = "journal")]
mod initial_property;
#[cfg(feature = "journal")]
mod property_history;
#[cfg(feature = "journal")]
mod property_preservation;

#[cfg(feature = "journal")]
pub use created::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError,
    CreatedDefaultZeroColumnRequired, RepeatedDefaultZeroAddColumnCandidate,
};

mod preservation;
mod source;

#[cfg(feature = "journal")]
pub use created::{CreatedAddColumnCandidate, CreatedAddColumnError};
#[cfg(feature = "journal")]
pub use created::{
    CreatedFieldAdditionSourcePurpose, FieldAdditionCandidate, FieldAdditionError,
    FieldAdditionTarget,
};

use super::{CreateDatabaseError, request};
use crate::{InspectError, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Refusal before an empty saved table receives a proposed column.
#[derive(Debug, Error)]
pub enum AddColumnError {
    /// Native inspection, resource, allocation or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The new declaration violates the existing constructive identifier limits.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// The actual source is outside the canonical empty keyless Int32 profile.
    #[error("database is outside the empty-table add-column source profile")]
    Source,
    /// Independent preservation or recaptured declaration checks failed.
    #[error("add-column candidate preservation check failed")]
    Preservation,
}

/// Complete source-bound candidate, deliberately separate from journalable plans.
///
/// No conversion supplies normal save, recovery, macro or filesystem authority.
/// A future original-intent operation must repeat this same source qualification.
pub struct AddColumnCandidate {
    before: NativeImage,
    after: Vec<u8>,
    table: String,
    column: String,
    columns: usize,
}

fn owned(text: &str, budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(text);
    Ok(result)
}

impl NativeImage {
    /// Add one ordinary nullable Int32 declaration to a canonical empty table.
    ///
    /// Actual source structure must equal the existing canonical single-table
    /// constructor for its decoded names: header 5, one through fifteen ordinary
    /// Int32 fields, no indexes/properties/history, and zero rows. Unknown bytes,
    /// other objects, nonzero native timestamps and broader layouts refuse.
    /// Names are validated without normalization. Only the selected definition's
    /// counts, length/free-space and appended declaration/name tail may change.
    /// Catalog timestamps and every other original byte remain intact.
    pub fn prepare_add_int32_column_candidate(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AddColumnCandidate, AddColumnError> {
        let _span = tracing::debug_span!("native.add_column.prepare_candidate").entered();
        check(budget, cancelled)?;
        request::name(column, budget, cancelled)?;
        let checked = source::capture(&self, table, budget, cancelled)?;
        let schema = checked.schema();
        let count = schema.columns().len();
        let mut names = source::names(schema, budget)?;
        names.push(column);
        let after = build::candidate(&self, schema.name(), &names, budget, cancelled)?;
        let table_name = owned(schema.name(), budget)?;
        let column_name = owned(column, budget)?;
        check(budget, cancelled)?;
        drop(names);
        Ok(AddColumnCandidate {
            before: self,
            after,
            table: table_name,
            column: column_name,
            columns: count + 1,
        })
    }
}

impl AddColumnCandidate {
    /// Complete immutable source image.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete candidate image, without journal or normal-save authority.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Original decoded table spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact new column spelling.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Resulting number of ordinary Int32 declarations.
    pub const fn column_count(&self) -> usize {
        self.columns
    }
}

#[cfg(test)]
mod tests;
