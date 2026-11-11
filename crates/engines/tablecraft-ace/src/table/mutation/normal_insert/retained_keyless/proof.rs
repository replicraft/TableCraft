//! Independently resolve original presence and reconstruct ordinary retained rows.
use super::{incrementing, qualify, retained};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    header::check,
    table::mutation::{
        insert_defaults,
        record_insertion::{proof::current as records, properties},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    original: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != original.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture_retained(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = retained::capture(&image, table, budget, cancelled)?;
    qualify(&image, &source, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let rows = insert_defaults::derive_retained(
        &image,
        source.table.schema(),
        original,
        budget,
        cancelled,
    )?;
    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
    let shape = records::capture_ordinary(source.table.schema(), budget, cancelled)?;
    let mut encoded = incrementing::vector(rows.len(), budget, cancelled)?;
    for row in &rows {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        encoded.push(records::reconstruct(
            source.table.schema(),
            &shape,
            row.values(),
            budget,
            cancelled,
        )?);
    }
    let (expected, _) = super::super::proof::reconstruct_retained_encoded(
        &image, &source, &rows, encoded, false, budget, cancelled,
    )?;
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    let after = NativeImage::capture_retained(
        &mut Cursor::new(expected.as_slice()),
        scope,
        budget,
        cancelled,
    )?;
    let final_source = retained::capture(&after, table, budget, cancelled)?;
    qualify(&after, &final_source, table, budget, cancelled)?;
    let total = source
        .records
        .len()
        .checked_add(original.len())
        .ok_or(InspectError::StructureLimit)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != total
        || usize::try_from(final_source.table.schema().declared_rows()).ok() != Some(total)
    {
        return Err(EditError::Preservation);
    }
    // Whole-image comparison above leaves LAST, all index/history bytes, old
    // records and retired presence bits outside the fresh row unchanged.
    check(budget, cancelled)?;
    Ok(())
}
