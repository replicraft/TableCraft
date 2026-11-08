//! Explicit OFF metadata on the original closed creator ledger, never a default.
mod properties;
#[cfg(test)]
mod tests;
mod verify;

use super::{
    AddTableError, CreateDatabaseError,
    add_table::source,
    catalog_page::{Page, PageError},
    definition::put32,
    record::{self, DataPage},
    stored_text,
};
use crate::{
    InspectError, NativeImage,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const CATALOG_DEFINITION: usize = 2;
const MAP_PAGE: usize = 3;
const CATALOG_PAGE: usize = 4;
const ORIGINAL_OBJECTS: usize = 8;
const DATABASE_ID: u32 = 0x0f00_0004;
const DATABASE_PARENT: u32 = 0x0f00_0002;
const PROPERTY_OWNED: usize = 16;
const PROPERTY_CANDIDATE: usize = 17;
const MAP_BYTES: usize = 13;
const MAP_HEADER_BYTES: usize = 5;
const MAP_PAGE_CAPACITY: usize = (MAP_BYTES - MAP_HEADER_BYTES) * u8::BITS as usize;
const LONG_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const LVAL_OWNER: usize = u32::from_le_bytes(*b"LVAL") as usize;
const COPY_BYTES: usize = 16 * 1024;

/// Refusal before an explicit Name AutoCorrect OFF candidate can be published.
#[derive(Debug, Error)]
pub enum DatabaseOptionsError {
    /// Checked inspection, cancellation, allocation or cumulative resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Existing native record framing refused its constructive input.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// The original is outside the unchanged seven-role creator source language.
    #[error("database is outside the explicit option source profile")]
    Source,
    /// A catalog append or represented EOF allocation cannot fit.
    #[error("database option construction exceeds existing catalog or map capacity")]
    Capacity,
    /// Independent source-complement or decoded option/identity proof failed.
    #[error("database option preservation check failed")]
    Preservation,
}

impl From<PageError> for DatabaseOptionsError {
    fn from(error: PageError) -> Self {
        match error {
            PageError::Inspect(error) => Self::Inspect(error),
            PageError::Source => Self::Source,
            PageError::Capacity => Self::Capacity,
        }
    }
}

fn source_error(error: AddTableError) -> DatabaseOptionsError {
    match error {
        AddTableError::Inspect(error) => DatabaseOptionsError::Inspect(error),
        AddTableError::Create(error) => DatabaseOptionsError::Create(error),
        AddTableError::Capacity => DatabaseOptionsError::Capacity,
        AddTableError::Preservation => DatabaseOptionsError::Preservation,
        AddTableError::Source | AddTableError::DuplicateName => DatabaseOptionsError::Source,
    }
}

/// An unpublished candidate, separate from all existing journal intent families.
/// It establishes neither a save capability nor broader rename/add-table admission.
pub struct NativeDatabaseOptionsPlan {
    before: NativeImage,
    after: Vec<u8>,
    property_page: u32,
}

impl NativeDatabaseOptionsPlan {
    /// Complete standard captured original, including every user/index byte.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete checked candidate with one explicit OFF metadata role/body.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// The new single-row external LvProp body, not an allocation authority.
    pub const fn property_page(&self) -> u32 {
        self.property_page
    }
}

impl NativeImage {
    /// Explicitly author disabled Track/Perform Name AutoCorrect database options.
    ///
    /// Missing metadata keeps its original meaning until this operation is
    /// requested. The initial source is one table in the exact canonical creator
    /// ledger, with no existing MSysDb. It preserves all user/index bytes, appends
    /// one catalog record and one external LVAL page, and rewrites only existing
    /// empty owned/candidate map cells. Existing eight-role option sources refuse;
    /// enabled settings, NameMaps, catalog growth and page reuse are not inferred.
    pub fn plan_disable_name_auto_correct(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeDatabaseOptionsPlan, DatabaseOptionsError> {
        let _span = tracing::debug_span!("native.database_options.disable_name_tracking").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        if self.catalog().catalog().objects().len() != ORIGINAL_OBJECTS {
            return Err(DatabaseOptionsError::Source);
        }
        source::require_name(&self, "MSysDb", budget, cancelled).map_err(source_error)?;
        for origin in self.catalog().origins() {
            check(budget, cancelled)?;
            if origin.native_id() == DATABASE_ID as i32 {
                return Err(DatabaseOptionsError::Source);
            }
        }
        let (geometry, legacy) =
            source::derive_geometry(&self, false, budget, cancelled).map_err(source_error)?;
        let original = source::qualify_geometry(&self, geometry, legacy, budget, cancelled)
            .map_err(source_error)?;
        let new_page = self.as_bytes().len() / PAGE_BYTES;
        if new_page >= MAP_PAGE_CAPACITY {
            return Err(DatabaseOptionsError::Capacity);
        }
        for slot in [PROPERTY_OWNED, PROPERTY_CANDIDATE] {
            let range = original
                .maps
                .records
                .get(slot)
                .ok_or(DatabaseOptionsError::Source)?;
            if self.as_bytes()
                [MAP_PAGE * PAGE_BYTES + range.start..MAP_PAGE * PAGE_BYTES + range.end]
                != [0; MAP_BYTES]
            {
                return Err(DatabaseOptionsError::Source);
            }
        }

        let length = self
            .as_bytes()
            .len()
            .checked_add(PAGE_BYTES)
            .ok_or(DatabaseOptionsError::Capacity)?;
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
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        after.resize(length, 0);

        let property_bytes = properties::encode(budget, cancelled)?;
        let catalog_start = CATALOG_PAGE * PAGE_BYTES;
        let catalog = &mut after[catalog_start..catalog_start + PAGE_BYTES];
        let mut layout = Page::capture(catalog, CATALOG_DEFINITION as u32, budget, cancelled)?;
        let record_bytes = record::catalog_length(stored_text::Plan::ascii_literal("MSysDb"))
            + LONG_DESCRIPTOR_BYTES;
        Page::charge_append(record_bytes, budget, cancelled)?;
        let range = layout.append(catalog, record_bytes)?;
        let new_page = u32::try_from(new_page).map_err(|_| InspectError::StructureLimit)?;
        record::database_properties_catalog(
            &mut catalog[range],
            DATABASE_ID,
            DATABASE_PARENT,
            reference(new_page, 0),
            property_bytes.len(),
            budget,
            cancelled,
        )?;
        put32(
            &mut after[CATALOG_DEFINITION * PAGE_BYTES..(CATALOG_DEFINITION + 1) * PAGE_BYTES],
            RECORD_COUNT_OFFSET,
            ORIGINAL_OBJECTS as u32 + 1,
        )?;

        for slot in [PROPERTY_OWNED, PROPERTY_CANDIDATE] {
            check(budget, cancelled)?;
            let map = &original.maps.records[slot];
            let at = MAP_PAGE * PAGE_BYTES + map.start + MAP_HEADER_BYTES + new_page as usize / 8;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            after[at] |= 1 << (new_page % 8);
        }
        let mut lval = DataPage::new(&mut after[new_page as usize * PAGE_BYTES..], LVAL_OWNER)?;
        lval.append(property_bytes.len(), budget, cancelled)?
            .copy_from_slice(&property_bytes);
        lval.finish()?;
        verify::candidate(
            &self,
            &after,
            new_page,
            &original.catalog,
            &original.maps,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(NativeDatabaseOptionsPlan {
            before: self,
            after,
            property_page: new_page,
        })
    }
}
