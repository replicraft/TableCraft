//! Logical-root deletion over positively selected retained schema histories.
#[cfg(test)]
mod tests;

use super::{Family, indexed, selection};
use crate::{
    EditError, FileFormat, NativeEditPlan, NativeImage, RowDelete, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_schema,
    table::mutation::{current_schema, normal_actions, preserved_source::Source},
};
use tablecraft_core::limits::ResourceBudget;

const SOURCE_PAGES: usize = 11;
const USER_DEFINITION: u32 = 7;

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    if !image.select_native_current_delete_source(table, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    capture_table_drop(image, table, budget, cancelled)
}

pub(in crate::table::mutation) fn capture_table_drop(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    current_schema::observe(image, table, budget, cancelled).map_err(|error| match error {
        current_schema::Error::Inspect(error) => EditError::Inspect(error),
        current_schema::Error::Native(error) => error,
        current_schema::Error::Source => EditError::Unsupported(WriteBlocker::RowLayout),
    })?;
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        super::CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if catalog.columns().len() != super::NATIVE_CATALOG_FIELDS {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    let source = normal_actions::capture_current_delete(image, table, budget, cancelled)?;
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    if !source.indexes.indexes().is_empty() {
        indexed::qualify(image, &source, budget, cancelled)?;
    }
    check(budget, cancelled)?;
    Ok(source)
}

impl NativeImage {
    /// Select retained native metadata before attempting either DELETE language.
    ///
    /// This is metadata inspection, not program, row or index qualification.
    pub fn select_native_current_delete_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        let header = self.catalog().header();
        if header.format() != FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
            || self.as_bytes().len() != SOURCE_PAGES * PAGE_BYTES
        {
            return Ok(false);
        }
        let schema = inspect_table_schema(
            &mut self.reader(),
            self.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        Ok(schema.definition_page() == USER_DEFINITION
            && schema.physical_layout().max_columns > schema.columns().len())
    }

    /// Capture original logical-root witnesses under the current DELETE source.
    ///
    /// Empty selections still qualify the original programs, rows and indexes.
    /// Returned records grant no mutation authority; planning repeats admission.
    pub fn prepare_current_row_deletes(
        &self,
        table: &str,
        ordinals: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowDelete>, EditError> {
        let _span = tracing::debug_span!("native.current.delete.select").entered();
        let source = capture(self, table, budget, cancelled)?;
        if ordinals.len() > source.records.len() {
            return Err(EditError::Conflict);
        }
        selection::prepare(
            self,
            &source,
            ordinals.iter().copied(),
            Family::Current,
            budget,
            cancelled,
        )
    }

    pub(crate) fn plan_current_row_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        super::plan(self, table, deletes, Family::Current, budget, cancelled)
    }
}

/// Compose the row/index effects of a separately qualified current table DROP.
/// Its private family retains historical rows without changing TCND1/2 selectors.
pub(in crate::table::mutation) fn compose_table_drop(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let source = capture_table_drop(image, table, budget, cancelled)?;
    let requests = selection::prepare(
        image,
        &source,
        0..source.records.len(),
        Family::TableDrop,
        budget,
        cancelled,
    )?;
    let after = super::compose(image, &source, &requests, budget, cancelled)?;
    super::proof::prove_images(
        super::proof::Images {
            before: image.as_bytes(),
            after: &after,
        },
        image.catalog().catalog().scope(),
        table,
        &requests,
        Family::TableDrop,
        budget,
        cancelled,
    )?;
    Ok(after)
}
