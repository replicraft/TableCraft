//! A source-bound, independently checked normal LONG index candidate.
mod effect;
pub(crate) use effect::{NativeIndexCreateEffect, NativeIndexCreateParts};
mod selection;
mod unique;
pub use unique::NativeUniqueLongIndexCandidate;
#[cfg(test)]
mod tests;

use super::index_creation::SecondaryIndexCreate;
#[cfg(test)]
use super::index_population_audit as audit;
use super::index_population_source as source;
use crate::{EditError, NativeImage};
use tablecraft_core::limits::ResourceBudget;

const LONG_ENTRY_BYTES: usize = 1 + size_of::<i32>() + 2 * size_of::<u32>();

/// Complete candidate bytes for an ordinary nullable LONG secondary index.
///
/// This is not a durable edit plan or a persistence capability. The original
/// image, source admission and independent whole-image proof remain bound to it.
/// No native Access application save qualification is implied.
pub struct NativeLongIndexCandidate {
    construction: source::Construction,
    policy: super::index_creation::IndexPolicy,
    before: NativeImage,
    after: NativeImage,
    table: String,
    column: String,
    native_table: i32,
    definition: u32,
    column_id: u16,
    descending: bool,
    index: tablecraft_schema::SchemaName,
    native: usize,
    logical: u32,
    physical: u32,
    rows: usize,
    root: u32,
    pages: usize,
    height: u8,
}

impl NativeLongIndexCandidate {
    pub(crate) fn is_unique_construction(&self) -> bool {
        matches!(self.policy, super::index_creation::IndexPolicy::UniqueLong)
    }

    pub(crate) fn is_fresh_construction(&self) -> bool {
        self.construction == source::Construction::Fresh
    }

    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete independently checked image; no mutable storage is exposed.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    pub(crate) fn after_shared_bytes(&self) -> bytes::Bytes {
        // NO_CLONE: The independent readonly host witness can outlive this candidate.
        // NativeImageSource already materialized the shared control allocation;
        // only an immutable handle is copied, never the database payload.
        self.after.bytes.clone()
    }

    /// Original qualified table name.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Original selected field name retained by the request.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Original signed native catalog table identity.
    pub const fn native_table_id(&self) -> i32 {
        self.native_table
    }

    /// Original table-definition root, separate from the new index root.
    pub const fn definition_page(&self) -> u32 {
        self.definition
    }

    /// Stored creation-time field identity, separate from native/display position.
    pub const fn column_id(&self) -> u16 {
        self.column_id
    }

    /// Requested native key direction; ROW ties remain ascending.
    pub const fn descending(&self) -> bool {
        self.descending
    }

    /// Requested native index name.
    pub fn index_name(&self) -> &str {
        self.index.as_str()
    }

    /// Actual native field identity, independent of display ordinal.
    pub fn native_number(&self) -> usize {
        self.native
    }

    /// New logical index identity.
    pub fn logical_index_number(&self) -> u32 {
        self.logical
    }

    /// New physical index identity, separate from logical aliases.
    pub fn physical_index_number(&self) -> u32 {
        self.physical
    }

    /// Complete original live-row population, including NULL keys.
    pub fn indexed_rows(&self) -> usize {
        self.rows
    }

    /// New root INDEX page.
    pub fn root_page(&self) -> u32 {
        self.root
    }

    /// Number of new INDEX pages, excluding usage-map storage.
    pub fn index_pages(&self) -> usize {
        self.pages
    }

    /// Root height; zero denotes a single leaf.
    pub fn height(&self) -> u8 {
        self.height
    }
}

impl NativeImage {
    /// Build and independently prove an ordinary nullable, nonunique LONG index.
    ///
    /// The retained source/program certificate protects all companion fields,
    /// existing indexes and payloads. The selected ordinary LONG must be
    /// unindexed. This operation appends a paid bottom-up tree and map storage;
    /// it does not return journal or save authority.
    ///
    /// # Errors
    /// Refuses unqualified sources, fields or active content; conflicting names;
    /// actual definition/allocation capacity; malformed input, caller budgets,
    /// cancellation, allocation failure or any independent preservation mismatch.
    pub fn prepare_retained_long_index_creation(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeLongIndexCandidate, EditError> {
        let _span = tracing::debug_span!("native.index.create_long").entered();
        self.build_retained_long_index(
            table,
            request,
            source::Mode::OriginalLong,
            budget,
            cancelled,
        )
    }

    /// Build the normal LONG tree with a fresh nonconflicting logical identity.
    /// Existing count-based identities stay unchanged when available; a collision
    /// chooses checked maximum-plus-one while preserving every existing identity.
    /// This explicit producer policy is not a Microsoft allocation-algorithm claim.
    ///
    /// # Errors
    /// Returns source/target, namespace, ownership, capacity, cancellation,
    /// resource, allocation, identity exhaustion or independent-proof failures.
    pub fn prepare_retained_fresh_long_index_creation(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeLongIndexCandidate, EditError> {
        let _span = tracing::debug_span!("native.index.create_fresh_long").entered();
        self.build_retained_long_index(table, request, source::Mode::FreshLong, budget, cancelled)
    }

    fn build_retained_long_index(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        mode: source::Mode,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeLongIndexCandidate, EditError> {
        if !matches!(mode, source::Mode::OriginalLong | source::Mode::FreshLong) {
            return Err(EditError::Preservation);
        }
        let built = super::index_population::build::<LONG_ENTRY_BYTES>(
            self, table, request, mode, budget, cancelled,
        )?;
        Ok(candidate(built))
    }
}

fn candidate(built: super::index_population::Built) -> NativeLongIndexCandidate {
    let super::index_population::Built {
        before,
        after,
        construction,
        policy,
        table,
        column,
        index,
        native_table,
        definition,
        native,
        column_id,
        descending,
        logical,
        physical,
        rows,
        root,
        pages,
        height,
        ..
    } = built;
    NativeLongIndexCandidate {
        construction,
        policy,
        before,
        after,
        table,
        column,
        native_table,
        definition,
        column_id,
        descending,
        index,
        native,
        logical,
        physical,
        rows,
        root,
        pages,
        height,
    }
}

#[cfg(test)]
mod history_tests;
