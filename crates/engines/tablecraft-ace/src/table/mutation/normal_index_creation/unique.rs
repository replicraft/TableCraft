//! Distinct unique secondary LONG construction; old candidates stay nonunique.
use super::{LONG_ENTRY_BYTES, NativeLongIndexCandidate, candidate};
use crate::{EditError, NativeImage, UniqueLongIndexCreate};
use tablecraft_core::limits::ResourceBudget;

/// Complete unique-secondary candidate, without journal or persistence authority.
pub struct NativeUniqueLongIndexCandidate {
    inner: NativeLongIndexCandidate,
}

impl NativeUniqueLongIndexCandidate {
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

    /// Original selected field name.
    pub fn column(&self) -> &str {
        self.inner.column()
    }

    /// Original signed native catalog table identity.
    pub fn native_table_id(&self) -> i32 {
        self.inner.native_table_id()
    }

    /// Original table-definition page, separate from the INDEX root.
    pub fn definition_page(&self) -> u32 {
        self.inner.definition_page()
    }

    /// Original selected field name.
    pub fn column_id(&self) -> u16 {
        self.inner.column_id()
    }

    /// Requested native key direction; ROW ties remain ascending.
    pub fn descending(&self) -> bool {
        self.inner.descending()
    }

    /// Original requested native index name.
    pub fn index_name(&self) -> &str {
        self.inner.index_name()
    }

    /// Actual native field number, separate from display ordinal.
    pub fn native_number(&self) -> usize {
        self.inner.native_number()
    }

    /// Source-bound logical index identity.
    pub fn logical_index_number(&self) -> u32 {
        self.inner.logical_index_number()
    }

    /// Source-bound physical tree identity, separate from logical aliases.
    pub fn physical_index_number(&self) -> u32 {
        self.inner.physical_index_number()
    }

    /// Complete live population, including stored NULL keys.
    pub fn indexed_rows(&self) -> usize {
        self.inner.indexed_rows()
    }

    /// New INDEX root page.
    pub fn root_page(&self) -> u32 {
        self.inner.root_page()
    }

    /// Number of new INDEX pages, excluding map storage.
    pub fn index_pages(&self) -> usize {
        self.inner.index_pages()
    }

    /// New root height; zero denotes a leaf.
    pub fn height(&self) -> u8 {
        self.inner.height()
    }

    pub(crate) fn into_parts(self) -> super::NativeIndexCreateParts {
        self.inner.into_parts()
    }
}

impl NativeImage {
    /// Build and independently prove a nullable unique manual LONG secondary index.
    /// Required, IgnoreNulls, PRIMARY and generated-value authority remain absent.
    ///
    /// # Errors
    /// Refuses unsupported source/programs/fields, duplicate non-NULL keys,
    /// conflicting identities, capacity, preservation, allocation, cancellation
    /// and explicit caller resource limits.
    pub fn prepare_retained_unique_long_index_creation(
        self,
        table: &str,
        request: &UniqueLongIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeUniqueLongIndexCandidate, EditError> {
        let _span = tracing::debug_span!("native.index.create_unique_long").entered();
        let built = crate::table::mutation::index_population::build::<LONG_ENTRY_BYTES>(
            self,
            table,
            request.ordinary(),
            crate::table::mutation::index_population_source::Mode::UniqueLong,
            budget,
            cancelled,
        )?;
        Ok(NativeUniqueLongIndexCandidate {
            inner: candidate(built),
        })
    }
}

#[cfg(test)]
mod tests;
