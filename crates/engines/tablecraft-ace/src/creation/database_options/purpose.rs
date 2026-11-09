//! Metadata chooses exactly one source proof, without granting write authority.
#[cfg(test)]
mod tests;

use super::{
    DatabaseOptionsError, FieldHistoryDatabaseOptionsError, ORIGINAL_OBJECTS, field_history, source,
};
use crate::{
    EditError, FieldAdditionError, FileFormat, InspectError, NativeImage, format::page::PAGE_BYTES,
    header::check, table::mutation::catalog_property::storage,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Selected native source language; it is neither a review nor a save capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseOptionsSourcePurpose {
    /// Original creator source with an empty LvProp column-owned page ledger.
    Creator,
    /// Already allocated LvProp column storage; full field-history proof is next.
    FieldHistory,
}

/// Metadata/source qualification failure without implicit profile retry.
#[derive(Debug, Error)]
pub enum DatabaseOptionsSourceError {
    /// Native read, cancellation, allocation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Actual column map/role observation refused its physical storage profile.
    #[error(transparent)]
    Storage(EditError),
    /// Known format or selected constructive source language is not admitted.
    #[error("database is outside the selected explicit option source profile")]
    Source,
    /// Original creator source proof failed outside its ordinary source refusal.
    #[error(transparent)]
    Creator(DatabaseOptionsError),
    /// Field-history source proof failed outside its ordinary source refusal.
    #[error(transparent)]
    FieldHistory(FieldHistoryDatabaseOptionsError),
}

impl From<EditError> for DatabaseOptionsSourceError {
    fn from(error: EditError) -> Self {
        match error {
            EditError::Inspect(error) => Self::Inspect(error),
            error => Self::Storage(error),
        }
    }
}

impl From<DatabaseOptionsError> for DatabaseOptionsSourceError {
    fn from(error: DatabaseOptionsError) -> Self {
        match error {
            DatabaseOptionsError::Inspect(error) => Self::Inspect(error),
            DatabaseOptionsError::Source => Self::Source,
            error => Self::Creator(error),
        }
    }
}

impl From<FieldHistoryDatabaseOptionsError> for DatabaseOptionsSourceError {
    fn from(error: FieldHistoryDatabaseOptionsError) -> Self {
        match error {
            FieldHistoryDatabaseOptionsError::Inspect(error)
            | FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Inspect(error)) => {
                Self::Inspect(error)
            }
            FieldHistoryDatabaseOptionsError::Source
            | FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Source) => Self::Source,
            error => Self::FieldHistory(error),
        }
    }
}

impl NativeImage {
    /// Select one native option source purpose from actual column-owned storage.
    ///
    /// This does not follow property bodies or infer that missing settings are
    /// enabled. A database role can own external rows while user LvProp is absent
    /// or inline. Every selected constructor must still prove its complete source.
    pub fn disable_name_auto_correct_source_purpose(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DatabaseOptionsSourcePurpose, DatabaseOptionsSourceError> {
        let _span = tracing::debug_span!("native.database_options.source_purpose").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        if self.catalog().header().format() != FileFormat::Ace2016
            || self.catalog().header().page_size() != PAGE_BYTES as u32
        {
            return Err(DatabaseOptionsSourceError::Source);
        }
        let pages = storage::pages(self, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(if pages.is_empty() {
            DatabaseOptionsSourcePurpose::Creator
        } else {
            DatabaseOptionsSourcePurpose::FieldHistory
        })
    }

    /// Inspect the one selected complete source without constructing an AFTER.
    ///
    /// The old check_disable_name_auto_correct_source retains its original source
    /// language. This separate normal boundary selects metadata once, then calls
    /// exactly that source proof. Placement/AFTER validation remains in review.
    pub fn check_disable_name_auto_correct_for_source(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DatabaseOptionsSourcePurpose, DatabaseOptionsSourceError> {
        let _span = tracing::debug_span!("native.database_options.selected_source").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        // Preserve readable Absent/Other observations on common ineligible
        // catalogs before asking for this writer's specialized map structure.
        if self.catalog().catalog().objects().len() != ORIGINAL_OBJECTS {
            return Err(DatabaseOptionsSourceError::Source);
        }
        let purpose = self.disable_name_auto_correct_source_purpose(budget, cancelled)?;
        match purpose {
            DatabaseOptionsSourcePurpose::Creator => {
                source::capture(self, budget, cancelled)?;
            }
            DatabaseOptionsSourcePurpose::FieldHistory => {
                field_history::check_source(self, budget, cancelled)?;
            }
        }
        check(budget, cancelled)?;
        Ok(purpose)
    }
}
