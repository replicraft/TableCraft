//! Metadata-only construction selection; the consuming planner retains authority.
use crate::{InspectError, NativeImage, header::check, inspect_table_indexes};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    /// Select fresh index construction only when the old count identity is occupied.
    /// This reads declarations, never target records or required programs, and
    /// grants no source or mutation authority. The chosen consuming native plan
    /// must independently requalify the complete original source.
    ///
    /// # Errors
    /// Propagates actual metadata, target lookup, allocation, cancellation and
    /// caller limits. A failure must not be retried as another construction mode.
    pub fn select_native_fresh_long_index_creation_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        check(budget, cancelled)?;
        let observed = inspect_table_indexes(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let count =
            u32::try_from(observed.indexes().len()).map_err(|_| InspectError::StructureLimit)?;
        for index in observed.indexes() {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, 1)?;
            if index.number() == count {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests;
