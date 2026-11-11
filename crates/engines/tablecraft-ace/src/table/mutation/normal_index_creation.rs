//! A source-bound, independently checked normal LONG index candidate.
mod audit;
mod maps;
mod source;

use super::index_creation::{
    SecondaryIndexCreate, buffer,
    definition::{self, CreatedDescriptor},
};
use crate::table::{
    indexes::mutation::bulk_long::Population,
    table_append::{Inventory, allocation::AllocationError},
};
use crate::{
    EditError, InspectError, NativeImage, NativeImageSource, format::page::PAGE_BYTES,
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Complete candidate bytes for an ordinary nullable LONG secondary index.
///
/// This is not a durable edit plan or a persistence capability. The original
/// image, source admission and independent whole-image proof remain bound to it.
/// No native Access application save qualification is implied.
pub struct NativeLongIndexCandidate {
    before: NativeImage,
    after: NativeImage,
    table: String,
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
    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }
    /// Complete independently checked image; no mutable storage is exposed.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }
    /// Original qualified table name.
    pub fn table(&self) -> &str {
        &self.table
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
        let original = source::capture(&self, table, request, budget, cancelled)?;
        let rows = original.content.records.len();
        let population = Population::capture(
            original.keys(budget, cancelled)?,
            original.native,
            original.descending,
            original.index_start,
            budget,
            cancelled,
        )?;
        let maps = maps::build(
            original.index_start,
            population.page_count(),
            budget,
            cancelled,
        )?;
        let inventory = Inventory::capture(&self, budget, cancelled)?;
        let allocation = inventory
            .plan_allocation(maps.after_pages, budget, cancelled)
            .map_err(allocation_error)?;
        let descriptor = CreatedDescriptor {
            column: original.native,
            descending: original.descending,
            counter: population.distinct(),
            physical: original.layout.physical_count,
            logical: original.layout.logical_count,
            usage: maps.usage,
            root: population.root(),
        };
        let definition = definition::compose(
            &self,
            &original.layout,
            &original.name,
            descriptor,
            budget,
            cancelled,
        )?;
        let length = usize::try_from(maps.after_pages)
            .map_err(|_| InspectError::StructureLimit)?
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let mut bytes = buffer::vector(length, budget, cancelled)?;
        for chunk in self
            .as_bytes()
            .chunks(crate::preservation::COPY_CHUNK_BYTES)
        {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            bytes.extend_from_slice(chunk);
        }
        let pages = population.build(original.layout.definition, budget, cancelled)?;
        for page in pages.into_iter().chain(maps.pages) {
            check(budget, cancelled)?;
            if page.number as usize * PAGE_BYTES != bytes.len() || page.bytes.len() != PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, PAGE_BYTES)
                .map_err(InspectError::from)?;
            bytes.extend_from_slice(&page.bytes);
        }
        if bytes.len() != length {
            return Err(EditError::Preservation);
        }
        allocation
            .apply(&mut bytes, budget, cancelled)
            .map_err(allocation_error)?;
        let start = original.layout.definition as usize * PAGE_BYTES;
        bytes[start..start + PAGE_BYTES].copy_from_slice(&definition);
        let scope = self.catalog().catalog().scope();
        let after = NativeImage::capture_shared_retained(
            NativeImageSource::from_vec(bytes, budget, cancelled)?,
            scope,
            budget,
            cancelled,
        )?;
        audit::prove(&self, &after, table, request, budget, cancelled)?;
        let table = String::from_utf8(buffer::copy(table.as_bytes(), budget, cancelled)?)
            .map_err(|_| InspectError::Malformed)?;
        Ok(NativeLongIndexCandidate {
            before: self,
            after,
            table,
            index: original.name,
            native: original.native,
            logical: original.layout.logical_count,
            physical: original.layout.physical_count,
            rows,
            root: population.root(),
            pages: population.page_count(),
            height: population.height(),
        })
    }
}

fn allocation_error(error: AllocationError) -> EditError {
    match error {
        AllocationError::Inspect(error) => error.into(),
        AllocationError::Capacity => EditError::Unsupported(crate::WriteBlocker::RowLayout),
        AllocationError::Source => EditError::Preservation,
    }
}
