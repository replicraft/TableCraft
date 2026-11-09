//! Explicit OFF after complete canonical ADD history, without old-intent admission.
mod source;
#[cfg(test)]
mod tests;
mod verify;

use super::{DATABASE_ID, DATABASE_PARENT, LONG_DESCRIPTOR_BYTES, properties};
use crate::{
    CreateDatabaseError, DatabaseOptionsError, EditError, FieldAdditionError, InspectError,
    NativeImage,
    creation::{record, stored_text},
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES},
    header::check,
    table::mutation::catalog_property::database_role,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const CATALOG_DEFINITION: usize = 2;
const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const SOURCE_OBJECTS: usize = 8;
const COPY_BYTES: usize = 16 * 1024;

/// Refusal from the distinct OFF-after-field-history candidate profile.
#[derive(Debug, Error)]
pub enum FieldHistoryDatabaseOptionsError {
    /// Inert native inspection, cancellation, allocation or cumulative limit.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The existing complete constructive field-history proof refused.
    #[error(transparent)]
    Field(#[from] FieldAdditionError),
    /// Actual owned native page placement refused.
    #[error(transparent)]
    Placement(#[from] EditError),
    /// The existing native catalog record encoder refused.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// The source is outside the missing-role, fitted shared-page profile.
    #[error("database is outside the OFF-after-field-history source profile")]
    Source,
    /// Candidate construction cannot fit checked native lengths.
    #[error("database option insertion cannot fit existing storage")]
    Capacity,
    /// Complete complement or reopened native identity/property proof failed.
    #[error("database option composition preservation failed")]
    Preservation,
}

impl From<DatabaseOptionsError> for FieldHistoryDatabaseOptionsError {
    fn from(error: DatabaseOptionsError) -> Self {
        match error {
            DatabaseOptionsError::Inspect(error) => Self::Inspect(error),
            DatabaseOptionsError::Create(error) => Self::Create(error),
            DatabaseOptionsError::Source => Self::Source,
            DatabaseOptionsError::Capacity => Self::Capacity,
            DatabaseOptionsError::Preservation => Self::Preservation,
        }
    }
}

/// A complete native candidate with no journal, review or persistence authority.
/// It cannot be consumed as the frozen original TCDO0001 plan.
pub struct FieldHistoryDatabaseOptionsCandidate {
    before: NativeImage,
    after: Vec<u8>,
    property: u32,
}

impl FieldHistoryDatabaseOptionsCandidate {
    /// Complete source-qualified original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete checked candidate, with every original field/index byte retained.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Actual native ROW reference of the new database settings body.
    pub const fn property_reference(&self) -> u32 {
        self.property
    }
}

impl NativeImage {
    /// Explicitly insert OFF settings after a completely qualified ADD history.
    ///
    /// This separate unpublished profile requires one empty generated-primary
    /// table, a missing database-settings role and one fitting existing property
    /// page 10 in an 11-page image. It preserves all historical bodies and user
    /// descriptors, appending a new visible catalog root and shared-column body.
    /// It does not widen TCDO0001, TCAC0001–0005 or rename source admission.
    pub fn prepare_disable_name_auto_correct_after_field_history(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldHistoryDatabaseOptionsCandidate, FieldHistoryDatabaseOptionsError> {
        let _span = tracing::debug_span!("native.database_options.after_field_history").entered();
        let source = source::capture(&self, budget, cancelled)?;
        let body = properties::encode(budget, cancelled)?;
        let length = record::catalog_length(stored_text::Plan::ascii_literal("MSysDb"))
            .checked_add(LONG_DESCRIPTOR_BYTES)
            .ok_or(FieldHistoryDatabaseOptionsError::Capacity)?;
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        let mut record = Vec::new();
        record
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        record.resize(length, 0);
        record::database_properties_catalog(
            &mut record,
            DATABASE_ID,
            DATABASE_PARENT,
            source.next_property,
            body.len(),
            budget,
            cancelled,
        )?;
        let placed = database_role::append(
            &self,
            &body,
            &record,
            source.next_property,
            budget,
            cancelled,
        )?;
        let length = self.as_bytes().len();
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        for chunk in self.as_bytes().chunks(COPY_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            after.extend_from_slice(chunk);
        }
        for (page, bytes) in [
            (CATALOG_PAGE, &placed.catalog_bytes),
            (PROPERTY_PAGE, &placed.property_bytes),
        ] {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            after[page * PAGE_BYTES..(page + 1) * PAGE_BYTES].copy_from_slice(bytes);
        }
        let object_count = u32::try_from(self.catalog().origins().len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(FieldHistoryDatabaseOptionsError::Capacity)?;
        crate::creation::definition::put32(
            &mut after[CATALOG_DEFINITION * PAGE_BYTES..(CATALOG_DEFINITION + 1) * PAGE_BYTES],
            RECORD_COUNT_OFFSET,
            object_count,
        )?;
        verify::candidate(&self, &after, &source, &placed, budget, cancelled)?;
        Ok(FieldHistoryDatabaseOptionsCandidate {
            before: self,
            after,
            property: placed.property,
        })
    }
}
