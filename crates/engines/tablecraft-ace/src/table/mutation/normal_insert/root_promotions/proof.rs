//! Fresh raw DATA/membership, both partitions/fences, maps and full-image proof.
mod keys;
mod maps;
mod wire;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_index_counter,
    table::{indexes::mutation::insertion, mutation::record_insertion::properties},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.before_bytes().len().checked_add(4 * PAGE_BYTES) != Some(plan.after_bytes().len())
        || plan.changed_rows() != requests.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture_action_source(&image, table, budget, cancelled)?;
    let facts = super::admission::qualify(&image, &source, table, requests, budget, cancelled)?;
    let indexes = [
        insertion::qualify_actions(facts.checked(), 1, budget, cancelled)?,
        insertion::qualify_actions(facts.checked(), 2, budget, cancelled)?,
    ];
    // Revalidate the original complete catalog/map ownership. Byte proof below
    // independently rereads each cell; it never consumes staged claim arrays.
    let _authority = super::authority::Authority::capture(
        &image,
        source.table.schema(),
        indexes,
        budget,
        cancelled,
    )?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::super::records::qualify_complete(source.table.schema(), requests, budget, cancelled)?;
    let (mut expected, references) = super::super::proof::reconstruct_with_references(
        &image, &source, requests, budget, cancelled,
    )?;
    if references.len() != requests.len() || source.table.rows().len() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let count = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    super::reserve_pages(&mut expected, 4, budget, cancelled)?;
    for physical in 0..3_u32 {
        check(budget, cancelled)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let column = usize::from(physical != 0);
        let field = &source.table.schema().columns()[column];
        if key.number() != field.number() {
            return Err(EditError::Preservation);
        }
        let descending = key.descending();
        let wide = physical != 0;
        let mut population = super::vector(count, budget, cancelled)?;
        for (record, row) in source.records.iter().zip(source.table.rows()) {
            check(budget, cancelled)?;
            let units = keys::original(
                &record.data,
                source.table.schema(),
                field,
                row.get(column).ok_or(EditError::Preservation)?,
                budget,
                cancelled,
            )?;
            population.push(keys::Key {
                units,
                reference: record.reference,
            });
        }
        keys::sort(&mut population, descending, budget, cancelled)?;
        let unique = index
            .constraints()
            .ok_or(InspectError::Unsupported)?
            .unique();
        let original_distinct = keys::distinct(&population, unique, budget, cancelled)?;
        let root = index.root_page() as usize * PAGE_BYTES;
        let page = image
            .as_bytes()
            .get(root..root + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        wire::require_original(page, &population, (wide, descending), budget, cancelled)?;
        for (request, &reference) in requests.iter().zip(&references) {
            check(budget, cancelled)?;
            let units = keys::requested(
                request
                    .values()
                    .get(column)
                    .ok_or(EditError::Preservation)?,
                field,
            )?;
            population.push(keys::Key { units, reference });
        }
        keys::sort(&mut population, descending, budget, cancelled)?;
        let final_distinct = keys::distinct(&population, unique, budget, cancelled)?;
        let delta = final_distinct
            .checked_sub(original_distinct)
            .ok_or(EditError::Preservation)?;
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            scope,
            table,
            index.name(),
            budget,
            cancelled,
        )?;
        if counter.definition_page() != source.table.schema().definition_page()
            || counter.physical_index() != physical
            || delta > requests.len()
        {
            return Err(EditError::Preservation);
        }
        let next = counter
            .value()
            .checked_add(u32::try_from(delta).map_err(|_| InspectError::StructureLimit)?)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        expected[counter.byte_range()].copy_from_slice(&next.to_le_bytes());
        if physical == 0 {
            wire::frame(
                &mut expected[root..root + PAGE_BYTES],
                &population,
                (false, false),
                0,
                budget,
                cancelled,
            )?;
        } else {
            let right = image
                .catalog()
                .header()
                .page_count()
                .checked_add((physical - 1) * 2)
                .ok_or(InspectError::StructureLimit)?;
            let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
            let middle = population.len() / 2;
            if middle == 0 {
                return Err(EditError::Preservation);
            }
            let leaves = wire::leaves(
                source.table.schema().definition_page(),
                (right, left),
                &population,
                descending,
                budget,
                cancelled,
            )?;
            wire::parent(
                &mut expected[root..root + PAGE_BYTES],
                (right, left),
                population[middle - 1],
                descending,
                budget,
                cancelled,
            )?;
            for page in leaves {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, PAGE_BYTES as u64)
                    .map_err(InspectError::from)?;
                expected.extend_from_slice(&page);
            }
        }
    }
    maps::reconstruct(&image, indexes, &mut expected, budget, cancelled)?;
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture_action_source(&after, table, budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != source.table.schema().definition_page()
        || final_source.records.len() != count
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
