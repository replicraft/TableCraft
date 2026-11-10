//! Logical-root deletion over a separately qualified retained storage cohort.
use super::{Family, indexed, selection};
use crate::{
    EditError, NativeEditPlan, NativeImage, RowDelete, WriteBlocker,
    header::check,
    table::mutation::{normal_actions, preserved_source::Source},
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
    let source = normal_actions::capture_retained_delete(image, table, budget, cancelled)?;
    if source.records.len() != source.table.rows().len()
        || usize::try_from(source.table.schema().declared_rows()).ok() != Some(source.records.len())
    {
        return Err(EditError::Preservation);
    }
    // The first retained DELETE writer owns one physical Int32 primary leaf.
    // NODE mutation and keyless tables need their own independent acceptance.
    if source.indexes.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    indexed::qualify(image, &source, Family::Retained, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(source)
}

impl NativeImage {
    /// Fully qualify retained deletion without issuing a mutation capability.
    pub fn check_native_retained_delete_table(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        capture(self, table, budget, cancelled)?;
        Ok(())
    }

    /// Capture exact original terminal payloads while keeping logical-root row
    /// ordinals distinct from hidden physical slots. Empty selections still
    /// perform complete source, constraint and INDEX admission.
    pub fn prepare_retained_row_deletes(
        &self,
        table: &str,
        ordinals: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RowDelete>, EditError> {
        let source = capture(self, table, budget, cancelled)?;
        if ordinals.len() > source.records.len() {
            return Err(EditError::Conflict);
        }
        selection::prepare(
            self,
            &source,
            ordinals.iter().copied(),
            Family::Retained,
            budget,
            cancelled,
        )
    }

    pub(crate) fn plan_retained_row_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        super::plan(self, table, deletes, Family::Retained, budget, cancelled)
    }
}

#[cfg(test)]
mod tests;
