//! Fresh original presence and independent signed arithmetic; no staged keys.
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert, header::check,
    table::mutation::record_insertion::properties,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

pub(in crate::table::mutation::normal_insert) fn prove(
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

    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture_source(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let counter = super::mode::capture(&image, &source, budget, cancelled)?;
    let derived =
        super::proof_rows::derive(&image, &source, &counter, original, budget, cancelled)?;
    let final_last = derived.last;
    let rows = derived.rows;
    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
    let (facts, optional) = super::super::indexed::qualify(
        &image,
        &source,
        table,
        &rows,
        super::super::indexed::LvalScope::Original,
        budget,
        cancelled,
    )?;
    if optional {
        return Err(EditError::Preservation);
    }

    let (mut expected, references) = super::super::proof::reconstruct_incrementing(
        &image,
        &source,
        &rows,
        counter.column,
        budget,
        cancelled,
    )?;
    if source.table.schema().layout.physical_indexes == 1 {
        let index = source
            .indexes
            .indexes()
            .first()
            .ok_or(EditError::Preservation)?;
        super::super::indexed::proof::reconstruct_index(
            &image,
            &source,
            index,
            super::super::indexed::proof::RequestedRows {
                requests: &rows,
                references: &references,
            },
            &mut expected,
            budget,
            cancelled,
        )?;
    } else {
        for physical in 0..source.table.schema().layout.physical_indexes {
            check(budget, cancelled)?;
            let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
            let index = crate::table::indexes::mutation::insertion::qualify_actions(
                facts.checked(),
                physical,
                budget,
                cancelled,
            )?;
            super::super::indexed::proof::reconstruct_index(
                &image,
                &source,
                index,
                super::super::indexed::proof::RequestedRows {
                    requests: &rows,
                    references: &references,
                },
                &mut expected,
                budget,
                cancelled,
            )?;
        }
    }
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    expected[counter.offset..counter.offset + size_of::<i32>()]
        .copy_from_slice(&final_last.to_le_bytes());
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture_source(&after, table, budget, cancelled)?;
    let final_counter = super::mode::capture(&after, &final_source, budget, cancelled)?;
    let (final_facts, _) = super::super::indexed::qualify(
        &after,
        &final_source,
        table,
        &[],
        super::super::indexed::LvalScope::Original,
        budget,
        cancelled,
    )?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != source.records.len() + original.len()
        || final_counter.last != final_last
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }

    check(budget, cancelled)?;
    Ok(())
}
