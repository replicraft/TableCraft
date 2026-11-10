//! Shared retained-table metadata selection before whole-image capture.
use crate::{
    CatalogInspection, FileFormat, InspectError, NativeImage, NativeReadSource, NativeTableStorage,
    format::{
        allocation,
        bytes::u16_at,
        reader::{Reader, row_count, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, slot_offset},
    },
    header::check,
    inspect_catalog, inspect_header, inspect_table_schema,
};
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const ORIGINAL_USER_DEFINITION: u32 = 7;
const CATALOG_NAME: &str = "MSysObjects";
const DATABASE_ROLE: &str = "MSysDb";

impl NativeImage {
    /// Observe the original retained-table storage cohort without granting row
    /// or schema mutation permission. Each consuming source proves its own purpose.
    pub fn has_retained_table_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        from_catalog(&mut self.reader(), self.catalog(), table, budget, cancelled)
    }

    /// Inspect the same original metadata selector before choosing Standard or
    /// Retained image ownership. This reads no target user records and grants no
    /// source, program, index or mutation authority. Consuming plans recheck it.
    pub fn select_retained_table_source<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, InspectError> {
        let header = inspect_header(source, budget, cancelled)?;
        if header.format() != FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Ok(false);
        }
        let mut source = Plain(source);
        let catalog = inspect_catalog(&mut source, scope, budget, cancelled)?;
        from_catalog(&mut source, &catalog, table, budget, cancelled)
    }
}

fn from_catalog<R: NativeReadSource>(
    source: &mut R,
    catalog: &CatalogInspection,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    check(budget, cancelled)?;
    let header = catalog.header();
    if header.format() != FileFormat::Ace2016 || header.subversion() != 0 || header.encoded_pages()
    {
        return Ok(false);
    }
    let mut database_role = false;
    let mut added_role = false;
    for (object, origin) in catalog.catalog().objects().iter().zip(catalog.origins()) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, object.name().as_str().len() as u64)
            .map_err(InspectError::from)?;
        if origin.is_system()
            && origin.type_code() == 2
            && object.name().as_str().eq_ignore_ascii_case(DATABASE_ROLE)
        {
            database_role = true;
        }
        if !origin.is_system()
            && origin.table_storage() == Some(NativeTableStorage::Local)
            && origin.native_id() > ORIGINAL_USER_DEFINITION as i32
        {
            added_role = true;
        }
    }
    if !database_role {
        return Ok(false);
    }
    let scope = catalog.catalog().scope();
    let target = inspect_table_schema(source, scope, table, budget, cancelled)?;
    if target.definition_page() > ORIGINAL_USER_DEFINITION || added_role {
        return Ok(true);
    }

    // A retained logical tombstone is metadata, not permission to ignore a
    // physical orphan. The consuming inventory must prove its complete role.
    let catalog_schema = inspect_table_schema(source, scope, CATALOG_NAME, budget, cancelled)?;
    let mut reader = Reader {
        source,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, catalog_schema.physical_layout().usage)?;
    for page_number in owned {
        let page = reader.page(page_number)?;
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            let word = u16_at(&page, slot_offset(slot))?;
            if word & (DELETED_MASK | RELOCATED_MASK) == (DELETED_MASK | RELOCATED_MASK) {
                row_range(&page, slot)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// Whole-image capture accepts plain Read+Seek sources. This selector observes
// those same unencoded bytes without manufacturing a captured image or owner.
struct Plain<'a, R>(&'a mut R);

impl<R: Read> Read for Plain<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.0.read(output)
    }
}

impl<R: Seek> Seek for Plain<'_, R> {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.0.seek(at)
    }
}

impl<R: Read + Seek> NativeReadSource for Plain<'_, R> {}

#[cfg(test)]
mod tests;
