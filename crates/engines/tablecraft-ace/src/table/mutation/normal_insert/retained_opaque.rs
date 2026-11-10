//! Fresh generated rows preserve the complete opaque storage of older records.
use super::{generated, retained};
use crate::{
    EditError, NativeEditPlan, NativeImage, OmittedRowInsert, WriteBlocker,
    header::check,
    table::mutation::{normal_ordinary, preserved_source::Source, record_serialization},
};
use tablecraft_core::limits::ResourceBudget;

fn observe(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    image.require_retained_capture()?;
    normal_ordinary::capture_retained_opaque_insert(image, table, budget, cancelled)
}

fn has_ambiguous(
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let mut found = false;
    for record in &source.records {
        check(budget, cancelled)?;
        found |= record_serialization::opaque_rows::is_ambiguous(
            source.table.schema(),
            &record.data,
            budget,
            cancelled,
        )?;
    }
    Ok(found)
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    if !image.has_retained_table_source(table, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let source = observe(image, table, budget, cancelled)?;
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    if !has_ambiguous(&source, budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(source)
}

impl NativeImage {
    /// Select opaque original geometry before choosing an INSERT source family.
    /// A failed legacy source proof is never retried through this operation.
    pub fn select_native_retained_opaque_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        if !self.has_retained_table_source(table, budget, cancelled)? {
            return Ok(false);
        }
        let source = observe(self, table, budget, cancelled)?;
        has_ambiguous(&source, budget, cancelled)
    }

    /// Qualify fresh generated rows while preserving every opaque original row.
    pub fn check_native_retained_opaque_insert_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let source = capture(self, table, budget, cancelled)?;
        retained::qualify(self, &source, table, budget, cancelled)
    }

    pub(crate) fn plan_retained_opaque_omitted_row_inserts(
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
            generated::Family::RetainedOpaque,
            budget,
            cancelled,
        )
    }
}
