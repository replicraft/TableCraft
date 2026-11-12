//! Fresh retained rows and growing INDEX trees; no producer keys or placements.
use super::{declarations, incrementing, qualify, retained};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    header::check,
    table::{
        indexes::mutation::insertion,
        mutation::{
            insert_defaults,
            normal_insert::indexed::proof::{RequestedRows, tree},
            record_insertion::{proof::current as records, properties},
        },
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
    declarations(&source, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    let rows = insert_defaults::derive_retained(
        &image,
        source.table.schema(),
        original,
        budget,
        cancelled,
    )?;
    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
    let facts = qualify(&image, &source, table, &rows, budget, cancelled)?;
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
    let (mut expected, references) = super::super::proof::reconstruct_retained_encoded(
        &image, &source, &rows, encoded, true, budget, cancelled,
    )?;
    for physical in 0..source.table.schema().physical_layout().physical_indexes {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        tree::reconstruct_retained_manual(
            tree::Operation {
                image: &image,
                source: &source,
                checked: facts.checked(),
                index,
                rows: RequestedRows {
                    requests: &rows,
                    references: &references,
                },
            },
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    let after = NativeImage::capture_retained(
        &mut Cursor::new(expected.as_slice()),
        scope,
        budget,
        cancelled,
    )?;
    let final_source = retained::capture(&after, table, budget, cancelled)?;
    let final_facts = qualify(&after, &final_source, table, &[], budget, cancelled)?;
    let total = source
        .records
        .len()
        .checked_add(original.len())
        .ok_or(InspectError::StructureLimit)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != total
        || usize::try_from(final_source.table.schema().declared_rows()).ok() != Some(total)
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
