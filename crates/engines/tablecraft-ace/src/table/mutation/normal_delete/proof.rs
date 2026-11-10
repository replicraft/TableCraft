//! Rebuild root-slot and row-count changes from the original image and requests.
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowDelete,
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        row::{
            DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::mutation::{fixed_records, record_roots},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

#[cfg(test)]
pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    deletes: &[RowDelete],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with_source(
        plan,
        scope,
        table,
        deletes,
        super::Family::Ordinary,
        budget,
        cancelled,
    )
}

pub(super) fn prove_with_source(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    deletes: &[RowDelete],
    family: super::Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = family.capture(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    family.unique(deletes, source.records.len(), budget, cancelled)?;
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != deletes.len()
        || source.records.len() != source.table.rows().len()
    {
        return Err(EditError::Preservation);
    }

    // Resolve the original graph again, independently of staged root choices.
    let captured =
        record_roots::capture(&image, source.table.schema(), deletes, budget, cancelled)?;
    let mut expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for root in captured.selected {
        check(budget, cancelled)?;
        let request = root.update;
        let record = source
            .records
            .get(request.row())
            .ok_or(EditError::Conflict)?;
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.reference != root.reference
            || record.offset != root.final_record.offset
            || record.data != root.final_record.data
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        if record.data != request.expected_bytes() {
            return Err(EditError::Conflict);
        }
        let page = reference_page(root.reference) as usize;
        let slot = usize::from(reference_slot(root.reference));
        let at = page
            .checked_mul(PAGE_BYTES)
            .and_then(|base| base.checked_add(slot_offset(slot)))
            .ok_or(EditError::Preservation)?;
        let original = u16_at(image.as_bytes(), at)?;
        if original != root.word || original & !(OFFSET_MASK | RELOCATED_MASK) != 0 {
            return Err(EditError::Preservation);
        }
        let tombstone = original | DELETED_MASK | RELOCATED_MASK;
        expected
            .get_mut(at..at + size_of::<u16>())
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&tombstone.to_le_bytes());
    }

    let schema = source.table.schema();
    let count_at = (schema.definition_page() as usize)
        .checked_mul(PAGE_BYTES)
        .and_then(|base| base.checked_add(RECORD_COUNT_OFFSET))
        .ok_or(EditError::Preservation)?;
    let before_count = u32_at(image.as_bytes(), count_at)?;
    let removed = u32::try_from(deletes.len()).map_err(|_| InspectError::StructureLimit)?;
    if before_count != schema.declared_rows() {
        return Err(EditError::Preservation);
    }
    let remaining = before_count
        .checked_sub(removed)
        .ok_or(EditError::Conflict)?;
    expected
        .get_mut(count_at..count_at + size_of::<u32>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&remaining.to_le_bytes());

    if !source.indexes.indexes().is_empty() {
        super::indexed::reconstruct(&image, &source, deletes, &mut expected, budget, cancelled)?;
    }

    // This is a new Source and complete final scalar constraint check, not the
    // UPDATE validator, whose invariant intentionally requires equal row counts.
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = family.capture(&after, table, budget, cancelled)?;
    if final_source.table.schema().object_id() != schema.object_id()
        || final_source.table.schema().definition_page() != schema.definition_page()
        || final_source.table.schema().declared_rows() != remaining
        || final_source.records.len() != source.records.len() - deletes.len()
    {
        return Err(EditError::Preservation);
    }
    for (wanted, observed) in expected
        .chunks(PAGE_BYTES)
        .zip(plan.after_bytes().chunks(PAGE_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, wanted.len() as u64)
            .map_err(InspectError::from)?;
        if wanted != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
