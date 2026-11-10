//! Generated rows over positively inventoried live and retired native storage.
#[cfg(test)]
mod tests;
use super::{generated, incrementing, indexed};
use crate::{
    EditError, FileFormat, InspectError, NativeEditPlan, NativeImage, NativeTableStorage,
    OmittedRowInsert, WriteBlocker,
    format::{
        allocation,
        bytes::u16_at,
        reader::{Reader, row_count, row_range},
        row::{DELETED_MASK, RELOCATED_MASK, slot_offset},
    },
    header::check,
    inspect_table_schema,
    table::mutation::{normal_ordinary, preserved_source::Source, record_insertion},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ORIGINAL_USER_DEFINITION: u32 = 7;
const CATALOG_NAME: &str = "MSysObjects";
const DATABASE_ROLE: &str = "MSysDb";

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    if !image.select_native_retained_insert_source(table, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    image.require_retained_capture()?;
    let source = normal_ordinary::capture_retained_insert(image, table, budget, cancelled)?;
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    Ok(source)
}

impl NativeImage {
    /// Select retained-storage INSERT from original catalog metadata, before any
    /// source admission. A failed legacy or current proof is never retried here.
    pub fn select_native_retained_insert_source(
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
        {
            return Ok(false);
        }
        let mut database_role = false;
        let mut added_role = false;
        for (object, origin) in self
            .catalog()
            .catalog()
            .objects()
            .iter()
            .zip(self.catalog().origins())
        {
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
        let scope = self.catalog().catalog().scope();
        let target = inspect_table_schema(&mut self.reader(), scope, table, budget, cancelled)?;
        if target.definition_page() > ORIGINAL_USER_DEFINITION || added_role {
            return Ok(true);
        }

        // A retained logical tombstone is metadata, not permission to ignore a
        // physical orphan. The consuming inventory must prove its complete role.
        let catalog =
            inspect_table_schema(&mut self.reader(), scope, CATALOG_NAME, budget, cancelled)?;
        let mut input = self.reader();
        let mut reader = Reader {
            source: &mut input,
            header,
            budget,
            cancelled,
            reads: 0,
        };
        let owned = allocation::pages(&mut reader, catalog.physical_layout().usage)?;
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

    /// Fully qualify retained generated-row insertion without authorizing a save.
    pub fn check_native_retained_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = capture(self, table, budget, cancelled)?;
        let counter = incrementing::mode::capture_current(self, &source, budget, cancelled)?;
        record_insertion::current::variables(
            source.table.schema(),
            counter.column,
            budget,
            cancelled,
        )?;
        incrementing::binding::validate(self, &source, counter.column, &[], budget, cancelled)?;
        indexed::qualify(self, &source, table, &[], budget, cancelled)?;
        Ok(())
    }

    pub(crate) fn plan_retained_omitted_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let source = capture(&self, table, budget, cancelled)?;
        generated::plan(
            self,
            source,
            table,
            requests,
            generated::Family::Retained,
            budget,
            cancelled,
        )
    }
}
