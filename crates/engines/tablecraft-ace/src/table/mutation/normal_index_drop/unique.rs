//! Unique-secondary removal has its own positive source and nominal candidate.
use super::{NativeIndexDropCandidate, source::Policy};
use crate::{EditError, InspectError, NativeImage, header::check, inspect_table_indexes};
use tablecraft_core::limits::{Resource, ResourceBudget};

const UNIQUE_SECONDARY_FLAGS: u8 = 0x81;

/// Independently proved unique-secondary removal, without durable authority.
pub struct NativeUniqueLongIndexDropCandidate {
    inner: NativeIndexDropCandidate,
}

impl NativeUniqueLongIndexDropCandidate {
    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.inner.before_bytes()
    }

    /// Complete independently proved replacement; no durable authority.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.inner.candidate_bytes()
    }

    /// Original qualified table name.
    pub fn table(&self) -> &str {
        self.inner.table()
    }

    /// Original requested native index name.
    pub fn index_name(&self) -> &str {
        self.inner.index_name()
    }

    /// Original signed native catalog table identity.
    pub fn native_table_id(&self) -> i32 {
        self.inner.native_table_id()
    }

    /// Original table-definition page, separate from the INDEX root.
    pub fn definition_page(&self) -> u32 {
        self.inner.definition_page()
    }

    /// Source-bound logical index identity.
    pub fn logical_index_number(&self) -> u32 {
        self.inner.logical_index_number()
    }

    /// Source-bound physical tree identity, separate from logical aliases.
    pub fn physical_index_number(&self) -> u32 {
        self.inner.physical_index_number()
    }

    /// Exclusively reclaimed tree/map pages; zero for a surviving alias.
    pub fn retired_pages(&self) -> usize {
        self.inner.retired_pages()
    }

    pub(crate) fn into_parts(self) -> super::NativeIndexDropParts {
        self.inner.into_parts()
    }
}

impl NativeImage {
    /// Select exact unique-secondary metadata before consuming DROP proof.
    /// This advisory result never grants retirement or journal authority.
    ///
    /// # Errors
    /// Returns metadata, cancellation, allocation and caller resource failures.
    pub fn select_native_retained_unique_long_index_drop(
        &self,
        table: &str,
        requested: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        let inspection = inspect_table_indexes(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let mut selected = false;
        let mut found = false;
        for index in inspection.indexes() {
            check(budget, cancelled)?;
            budget.charge(
                Resource::Steps,
                index.name().len().max(requested.len()) as u64,
            )?;
            if index.name().eq_ignore_ascii_case(requested) {
                if found {
                    return Err(InspectError::Malformed);
                }
                found = true;
                selected = index.flags() == UNIQUE_SECONDARY_FLAGS && index.native_type() == 0;
            }
        }
        Ok(selected)
    }

    /// Requalify and remove one exact unique-secondary manual LONG index.
    /// Surviving aliases retain their shared tree; exclusive retirement is proved.
    ///
    /// # Errors
    /// Refuses source/programs/dependencies, unsupported declarations, ownership,
    /// retirement geometry, preservation, cancellation, allocation and resources.
    pub fn prepare_retained_unique_long_index_drop(
        self,
        table: &str,
        index: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeUniqueLongIndexDropCandidate, EditError> {
        let inner =
            self.build_retained_index_drop(table, index, Policy::UniqueLong, budget, cancelled)?;
        Ok(NativeUniqueLongIndexDropCandidate { inner })
    }
}

#[cfg(test)]
mod tests;
