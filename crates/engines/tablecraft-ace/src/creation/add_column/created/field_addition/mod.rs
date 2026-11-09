//! Typed Number/Short Text addition with its own positive whole-source proof.
mod after_off;
mod build;
pub use after_off::{
    FieldAdditionAfterOffCandidate, FieldAdditionAfterOffError,
    FieldAdditionAfterOffHistoryCandidate, FieldAdditionAfterOffHistoryError,
};
mod purpose;
pub use purpose::CreatedFieldAdditionSourcePurpose;
mod source;
mod target;
#[cfg(test)]
mod tests;
mod verify;

pub use target::FieldAdditionTarget;

use super::super::{
    AddColumnCandidate, initial_property::InitialPropertyError, owned,
    property_history::PropertyHistoryError, property_preservation::PropertyPreservationError,
};
use crate::{
    CreateColumn, CreateDatabaseError, EditError, InspectError, NativeImage, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Refusal from the separate typed field-addition candidate language.
#[derive(Debug, Error)]
pub enum FieldAdditionError {
    /// Native inspection, cancellation, allocation or cumulative resource limit.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The explicit declaration is invalid or cannot fit the constructive layout.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// Checked native catalog placement or owned property storage refused.
    #[error(transparent)]
    Placement(#[from] EditError),
    /// The original source is outside this bounded mixed declaration history.
    #[error("source is outside the typed field-addition profile")]
    Source,
    /// Complete candidate preservation or reopened metadata proof failed.
    #[error("typed field-addition preservation check failed")]
    Preservation,
}

impl From<super::CreatedAddColumnError> for FieldAdditionError {
    fn from(error: super::CreatedAddColumnError) -> Self {
        match error {
            super::CreatedAddColumnError::Inspect(error) => Self::Inspect(error),
            super::CreatedAddColumnError::Declaration(error) => Self::Declaration(error),
            super::CreatedAddColumnError::Source => Self::Source,
            super::CreatedAddColumnError::Preservation => Self::Preservation,
        }
    }
}

impl From<PropertyHistoryError> for FieldAdditionError {
    fn from(error: PropertyHistoryError) -> Self {
        match error {
            PropertyHistoryError::Inspect(error) => Self::Inspect(error),
            PropertyHistoryError::Placement(error) => Self::Placement(error),
            PropertyHistoryError::Source => Self::Source,
        }
    }
}

impl From<PropertyPreservationError> for FieldAdditionError {
    fn from(error: PropertyPreservationError) -> Self {
        match error {
            PropertyPreservationError::Inspect(error) => Self::Inspect(error),
            PropertyPreservationError::Preservation => Self::Preservation,
        }
    }
}

impl From<InitialPropertyError> for FieldAdditionError {
    fn from(error: InitialPropertyError) -> Self {
        match error {
            InitialPropertyError::Inspect(error) => Self::Inspect(error),
            InitialPropertyError::Declaration(error) => Self::Declaration(error),
            InitialPropertyError::Placement(error) => Self::Placement(error),
            InitialPropertyError::Preservation => Self::Preservation,
        }
    }
}

/// Source-bound complete candidate with no journal, recovery or save authority.
/// This type does not widen any existing TCAC0001 through TCAC0004 request.
pub struct FieldAdditionCandidate {
    image: AddColumnCandidate,
    target: FieldAdditionTarget,
}

impl NativeImage {
    /// Prove the complete typed source history without constructing an addition.
    pub fn check_created_field_addition_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FieldAdditionError> {
        source::capture(self, table, budget, cancelled)?;
        Ok(())
    }

    /// Add an explicit nullable Number or sized Short Text declaration.
    ///
    /// Source admission reconstructs a canonical empty generated-primary seed
    /// and every independently represented property-history stage. Unknown keys,
    /// programs, other objects, populated tables and broader storage refuse.
    /// Existing fields, compression permissions, property dictionaries and old
    /// property rows remain exact. This candidate grants no normal save authority.
    pub fn prepare_created_add_field_candidate(
        self,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionCandidate, FieldAdditionError> {
        let _span = tracing::debug_span!("native.typed_field_addition.prepare_candidate").entered();
        check(budget, cancelled)?;
        crate::creation::request::name(column, budget, cancelled)?;
        let appended = target.declaration(column)?;
        let source = source::capture(&self, table, budget, cancelled)?;
        let mut columns = source.definition.borrowed_columns(budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)
            .map_err(InspectError::from)?;
        columns
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        columns.push(appended);
        let order = build::extend_order(source.order, &columns, target, budget, cancelled)?;
        let after = build::append(
            &self,
            build::Addition {
                checked: &source.checked,
                columns: &columns,
                order,
                external: source.external,
                target,
            },
            budget,
            cancelled,
        )?;
        let table = owned(source.checked.schema().name(), budget)?;
        let column = owned(column, budget)?;
        let count = columns.len();
        drop(columns);
        check(budget, cancelled)?;
        Ok(FieldAdditionCandidate {
            image: AddColumnCandidate {
                before: self,
                after,
                table,
                column,
                columns: count,
            },
            target,
        })
    }
}

impl FieldAdditionCandidate {
    /// Complete original image whose typed history was proved.
    pub fn before_bytes(&self) -> &[u8] {
        self.image.before_bytes()
    }

    /// Complete proposed image, without mutation authority.
    pub fn after_bytes(&self) -> &[u8] {
        self.image.after_bytes()
    }

    /// Exact source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.image.table()
    }

    /// Exact proposed field spelling.
    pub fn column(&self) -> &str {
        self.image.column()
    }

    /// Field count after the proposed addition.
    pub const fn column_count(&self) -> usize {
        self.image.column_count()
    }

    /// Explicit semantic declaration, independent of physical storage selection.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.target
    }
}

mod selected_source;
pub use selected_source::{
    FieldAdditionSourceAvailability, FieldAdditionSourceError, FieldAdditionSourcePurpose,
};
