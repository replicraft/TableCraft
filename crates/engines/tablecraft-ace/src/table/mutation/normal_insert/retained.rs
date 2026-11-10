//! Generated rows over positively inventoried live and retired native storage.
#[cfg(test)]
mod tests;
use super::{generated, incrementing, indexed};
use crate::{
    EditError, NativeEditPlan, NativeImage, OmittedRowInsert, WriteBlocker,
    table::mutation::{normal_ordinary, preserved_source::Source, record_insertion},
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    if !image.has_retained_table_source(table, budget, cancelled)? {
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
