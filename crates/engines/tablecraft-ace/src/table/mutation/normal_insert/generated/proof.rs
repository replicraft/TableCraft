//! Fresh original presence, scalar arithmetic, row bytes and physical placement.
use super::super::{incrementing, indexed};
use super::Family;
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    header::check,
    table::{
        indexes::mutation::insertion,
        mutation::record_insertion::{proof::current as records, properties},
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
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != original.len()
    {
        return Err(EditError::Preservation);
    }
    let image = capture_image(
        family,
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = family.capture(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let counter = incrementing::mode::capture_current(&image, &source, budget, cancelled)?;
    let derived =
        incrementing::proof_rows::derive(&image, &source, &counter, original, budget, cancelled)?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        &derived.rows,
        budget,
        cancelled,
    )?;
    let (facts, optional) =
        indexed::qualify(&image, &source, table, &derived.rows, budget, cancelled)?;
    if optional {
        return Err(EditError::Preservation);
    }

    let shape = records::capture(source.table.schema(), counter.column, budget, cancelled)?;
    let mut encoded = incrementing::vector(derived.rows.len(), budget, cancelled)?;
    for row in &derived.rows {
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
    let reconstruct = match family {
        Family::Current => super::super::proof::reconstruct_encoded,
        Family::Retained => super::super::proof::reconstruct_retained_encoded,
    };
    let (mut expected, references) = reconstruct(
        &image,
        &source,
        &derived.rows,
        encoded,
        true,
        budget,
        cancelled,
    )?;
    for physical in 0..source.table.schema().physical_layout().physical_indexes {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        indexed::proof::reconstruct_index(
            &image,
            &source,
            index,
            indexed::proof::RequestedRows {
                requests: &derived.rows,
                references: &references,
            },
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    expected[counter.offset..counter.offset + size_of::<i32>()]
        .copy_from_slice(&derived.last.to_le_bytes());
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    let after = capture_image(
        family,
        &mut Cursor::new(expected.as_slice()),
        scope,
        budget,
        cancelled,
    )?;
    let final_source = family.capture(&after, table, budget, cancelled)?;
    let final_counter =
        incrementing::mode::capture_current(&after, &final_source, budget, cancelled)?;
    let (final_facts, _) = indexed::qualify(&after, &final_source, table, &[], budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != source.records.len() + original.len()
        || final_counter.last != derived.last
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn capture_image(
    family: Family,
    input: &mut Cursor<&[u8]>,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeImage, EditError> {
    match family {
        Family::Current => Ok(NativeImage::capture(input, scope, budget, cancelled)?),
        Family::Retained => Ok(NativeImage::capture_retained(
            input, scope, budget, cancelled,
        )?),
    }
}
