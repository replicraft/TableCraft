//! Source-bound append facts retain old storage instead of inferring creator geometry.
pub(crate) mod allocation;
mod catalog;
mod selection;
mod storage;

use crate::{EditError, NativeImage, NativeSortOrder, header::check};
use tablecraft_core::limits::ResourceBudget;

/// Complete immutable source observation; it grants no record overwrite authority.
pub(crate) struct Inventory<'image> {
    image: &'image NativeImage,
    before_pages: u32,
    catalog_definition: u32,
    catalog_pages: Vec<u32>,
    map_pages: Vec<u32>,
    catalog_live_rows: u32,
    tables_parent: i32,
    catalog_name_order: NativeSortOrder,
}

impl<'image> Inventory<'image> {
    /// Observe the complete retained cohort using paid, owned row traversal.
    /// Each operation still requires its own source/program and mutation proof.
    pub(crate) fn capture(
        image: &'image NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        image.require_retained_capture()?;
        let facts = catalog::capture(image, budget, cancelled)?;
        let storage = storage::capture(image, &facts.schema, budget, cancelled)?;
        Ok(Self {
            image,
            before_pages: image.catalog().header().page_count(),
            catalog_definition: facts.schema.definition_page(),
            catalog_pages: storage.catalog,
            map_pages: storage.maps,
            catalog_live_rows: facts.schema.declared_rows(),
            tables_parent: facts.tables_parent,
            catalog_name_order: facts.name_order,
        })
    }

    /// Observe the known packed catalog without inventing an MSysDb role.
    /// Physical ownership uses the same complete census as retained append;
    /// the distinct catalog predicate keeps the source cohorts separate.
    pub(crate) fn capture_packed(
        image: &'image NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        image.require_retained_capture()?;
        let facts = catalog::capture_packed(image, budget, cancelled)?;
        let storage = storage::capture(image, &facts.schema, budget, cancelled)?;
        Ok(Self {
            image,
            before_pages: image.catalog().header().page_count(),
            catalog_definition: facts.schema.definition_page(),
            catalog_pages: storage.catalog,
            map_pages: storage.maps,
            catalog_live_rows: facts.schema.declared_rows(),
            tables_parent: facts.tables_parent,
            catalog_name_order: facts.name_order,
        })
    }

    /// Native-count creation separately proves represented indirect catalog
    /// candidates. Older retained action sources continue through `capture`.
    pub(crate) fn capture_native(
        image: &'image NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        image.require_retained_capture()?;
        let facts = catalog::capture_native(image, budget, cancelled)?;
        let storage = storage::capture(image, &facts.schema, budget, cancelled)?;
        Ok(Self {
            image,
            before_pages: image.catalog().header().page_count(),
            catalog_definition: facts.schema.definition_page(),
            catalog_pages: storage.catalog,
            map_pages: storage.maps,
            catalog_live_rows: facts.schema.declared_rows(),
            tables_parent: facts.tables_parent,
            catalog_name_order: facts.name_order,
        })
    }

    pub(crate) fn require_source(&self, image: &NativeImage) -> Result<(), EditError> {
        if !std::ptr::eq(self.image, image) {
            return Err(EditError::Preservation);
        }
        Ok(())
    }

    pub(crate) fn before_pages(&self) -> u32 {
        self.before_pages
    }

    pub(crate) fn catalog_definition(&self) -> u32 {
        self.catalog_definition
    }

    pub(crate) fn catalog_pages(&self) -> &[u32] {
        &self.catalog_pages
    }

    pub(crate) fn map_pages(&self) -> &[u32] {
        &self.map_pages
    }

    pub(crate) fn catalog_live_rows(&self) -> u32 {
        self.catalog_live_rows
    }

    pub(crate) fn tables_parent(&self) -> i32 {
        self.tables_parent
    }

    pub(crate) fn catalog_name_order(&self) -> NativeSortOrder {
        self.catalog_name_order
    }
}

fn refused() -> EditError {
    EditError::Unsupported(crate::WriteBlocker::ActiveContent)
}

#[cfg(test)]
mod tests;
