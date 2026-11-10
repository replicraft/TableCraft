//! Generated insertion over current sparse metadata, separate from legacy TCNA2.
#[cfg(test)]
mod tests;

use super::{incrementing, indexed};
use crate::{
    EditError, FileFormat, NativeEditPlan, NativeImage, OmittedRowInsert, WriteBlocker,
    format::{data_type::INT32, page::PAGE_BYTES},
    header::check,
    inspect_table_schema,
    table::mutation::{
        current_schema, normal_ordinary, preserved_source::Source, record_insertion,
    },
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
    if !image.select_native_current_insert_source(table, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    current_schema::observe(image, table, budget, cancelled).map_err(|error| match error {
        current_schema::Error::Inspect(error) => EditError::Inspect(error),
        current_schema::Error::Native(error) => error,
        current_schema::Error::Source => EditError::Unsupported(WriteBlocker::RowLayout),
    })?;
    let source = normal_ordinary::capture_current_insert(image, table, budget, cancelled)?;
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    Ok(source)
}

impl NativeImage {
    /// Original metadata selects this source before any failed legacy attempt.
    pub fn select_native_current_insert_source(
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
        if schema.definition_page() != USER_DEFINITION
            || schema.physical_layout().max_columns <= schema.columns().len()
        {
            return Ok(false);
        }
        if schema.physical_layout().variable_slots != 0 {
            return Ok(true);
        }
        for field in schema.columns() {
            check(budget, cancelled)?;
            if !field.fixed() || field.native_type() != INT32 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Explicit full source qualification; metadata-only hosts need not call it.
    pub fn check_native_current_insert_table(
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

    pub(crate) fn plan_current_omitted_row_inserts(
        self,
        table: &str,
        requests: &[OmittedRowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let source = capture(&self, table, budget, cancelled)?;
        super::generated::plan(
            self,
            source,
            table,
            requests,
            super::generated::Family::Current,
            budget,
            cancelled,
        )
    }
}
