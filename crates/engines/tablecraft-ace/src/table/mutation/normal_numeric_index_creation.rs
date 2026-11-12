//! Ordinary numeric index population; the candidate itself grants no save authority.
pub(crate) mod effect;
use super::{
    index_creation::SecondaryIndexCreate,
    index_population::{self, Built},
    index_population_source,
};
use crate::{EditError, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const NUMERIC_ENTRY_BYTES: usize = 1 + size_of::<i64>() + 2 * size_of::<u32>();

/// Independently checked ordinary single numeric index bytes over one original.
/// This type is separate from LONG candidates and grants no persistence or
/// conversion into an existing index-creation original language.
pub struct NativeNumericIndexCandidate {
    built: Built,
}

impl NativeNumericIndexCandidate {
    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.built.before.as_bytes()
    }

    /// Complete independently checked AFTER image, never mutable storage.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.built.after.as_bytes()
    }

    /// Original requested table spelling.
    pub fn table(&self) -> &str {
        &self.built.table
    }

    /// Original requested ordinary field spelling.
    pub fn column(&self) -> &str {
        &self.built.column
    }

    /// Exact independently qualified selected scalar kind.
    pub fn scalar_type(&self) -> ScalarType {
        self.built.scalar
    }

    /// Requested index name, qualified against the native index namespace.
    pub fn index_name(&self) -> &str {
        self.built.index.as_str()
    }

    /// Requested key direction; physical ROW ties remain ascending.
    pub fn descending(&self) -> bool {
        self.built.descending
    }

    /// Original signed native catalog table identity.
    pub fn native_table_id(&self) -> i32 {
        self.built.native_table
    }

    /// Actual source table-definition root.
    pub fn definition_page(&self) -> u32 {
        self.built.definition
    }

    /// Original native field number, independent from dense display position.
    pub fn native_number(&self) -> usize {
        self.built.native
    }

    /// Stored creation-time native field identity.
    pub fn column_id(&self) -> u16 {
        self.built.column_id
    }

    /// Fresh logical identity preserving every existing logical alias.
    pub fn logical_index_number(&self) -> u32 {
        self.built.logical
    }

    /// Appended physical identity, separate from the logical namespace.
    pub fn physical_index_number(&self) -> u32 {
        self.built.physical
    }

    /// Complete original population, including NULL where the native kind allows it.
    pub fn indexed_rows(&self) -> usize {
        self.built.rows
    }

    /// New physical root INDEX page.
    pub fn root_page(&self) -> u32 {
        self.built.root
    }

    /// Number of new INDEX pages, excluding represented map storage.
    pub fn index_pages(&self) -> usize {
        self.built.pages
    }

    /// Root height; zero denotes a single leaf.
    pub fn height(&self) -> u8 {
        self.built.height
    }
}

impl NativeImage {
    /// Build an ordinary nonunique single numeric index from actual stored values.
    /// The source retains complete program, dependency, index and allocation
    /// qualifications. Boolean keeps its two-value native order; Currency and
    /// IEEE/DATE use existing exact native order atoms without SQL conversion.
    /// No caller supplies absence or writer-admission flags, and this candidate
    /// cannot be consumed by the frozen LONG original-intent constructors.
    ///
    /// # Errors
    /// Refuses unsupported source/declaration/index policies, indexed/generated
    /// targets, malformed records or identities, namespace collisions, capacity,
    /// caller budgets, cancellation, allocation and independent proof failures.
    pub fn prepare_retained_numeric_index_creation(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeNumericIndexCandidate, EditError> {
        let _span = tracing::debug_span!("native.index.create_numeric").entered();
        let built = index_population::build::<NUMERIC_ENTRY_BYTES>(
            self,
            table,
            request,
            index_population_source::Mode::Numeric,
            budget,
            cancelled,
        )?;
        Ok(NativeNumericIndexCandidate { built })
    }
}

#[cfg(test)]
mod tests;
