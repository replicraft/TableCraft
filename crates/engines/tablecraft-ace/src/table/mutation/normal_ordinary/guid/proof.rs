//! Independent complete DATA, GUID/ROW membership and source-relative page replay.
mod keys;
use super::super::capture;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage,
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::{
        indexes::mutation::normal_index_audit::replacement,
        mutation::{
            fixed_records::{self, indexed::guid_updates::GuidUpdateIndexes, proof as records},
            preserved_source::validation,
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
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    let facts = GuidUpdateIndexes::capture(&image, &source, budget, cancelled)?;
    let expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    let (mut expected, changed_rows) = records::reconstruct_guid_updates_original(
        records::Original {
            image: &image,
            source: &source,
        },
        expected,
        groups,
        &facts,
        budget,
        cancelled,
    )?;
    for (position, selected) in facts.checked().indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = selected.index();
        if facts.checked().indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let tree =
            facts
                .ownership
                .require(&image, source.table.schema(), index, budget, cancelled)?;
        let population = keys::capture(&source, index, groups, budget, cancelled)?;
        tree.original(&population.original, budget, cancelled)?;
        replacement::reconstruct(
            &image,
            tree,
            &population.original,
            &population.final_keys,
            &mut expected,
            budget,
            cancelled,
        )?;
    }
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    // Complete AFTER Source is independently captured from reconstructed bytes;
    // no producer patches, keys, permission or AFTER metadata enter this proof.
    let final_image = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = capture(&final_image, table, budget, cancelled)?;
    let final_facts = GuidUpdateIndexes::capture(&final_image, &final_source, budget, cancelled)?;
    if final_facts.checked().indexes().len() != facts.checked().indexes().len()
        || final_source.records.len() != source.records.len()
        || final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || changed_rows != plan.changed_rows()
        || expected.len() != plan.after_bytes().len()
    {
        return Err(EditError::Preservation);
    }
    for (old, new) in source.records.iter().zip(&final_source.records) {
        check(budget, cancelled)?;
        if old.reference != new.reference
            || old.offset != new.offset
            || old.data.len() != new.data.len()
        {
            return Err(EditError::Preservation);
        }
    }
    for (expected, observed) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(plan.after_bytes().chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != observed {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
