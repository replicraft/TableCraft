//! One original-bound DATA reservation followed by two INDEX reservations.
mod hints;

use super::super::placement::Staged;
use super::{admission, authority, proof, selection};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_index_counter,
    table::{
        indexes::mutation::insertion::{
            self, InsertRow, root_split::prepare_root_promotion_after_data,
        },
        mutation::{fixed_records, preserved_source::Source},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Reservation {
    pub(super) records: Vec<Vec<u8>>,
    pub(super) placement: Staged,
}

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    requests: &[RowInsert],
    reserved: Reservation,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.ordinary.insert.compound_root_split.plan").entered();
    let Reservation { records, placement } = reserved;
    if !selection::selected(&image, &source, requests)?
        || requests.len() != 1
        || records.len() != 1
        || !placement.uses_eof()
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let (facts, _) = admission::qualify(&image, &source, table, requests, budget, cancelled)?;
    let index = insertion::qualify(facts.checked(), 0, budget, cancelled)?;
    let hint = hints::Hint::capture(
        &image,
        source.table.schema(),
        records[0].len(),
        budget,
        cancelled,
    )?;
    budget
        .charge_allocation(requests.len() as u64, size_of::<InsertRow<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    for (reference, request) in placement.references().zip(requests) {
        check(budget, cancelled)?;
        rows.push(InsertRow {
            reference,
            values: request.values(),
        });
    }
    let authority = authority::Authority::capture_after_data(
        &image,
        source.table.schema(),
        index,
        budget,
        cancelled,
    )?;
    authority.require(&image, source.table.schema(), index)?;
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        index.name(),
        budget,
        cancelled,
    )?;
    if counter.definition_page() != source.table.schema().definition_page()
        || counter.physical_index() != index.data_number()
    {
        return Err(EditError::Preservation);
    }
    let next = counter
        .value()
        .checked_add(u32::try_from(requests.len()).map_err(|_| InspectError::StructureLimit)?)
        .ok_or(InspectError::StructureLimit)?;
    let promotion =
        prepare_root_promotion_after_data(&image, facts.checked(), &rows, budget, cancelled)?;
    let root = index.root_page() as usize * PAGE_BYTES;
    let counter_range = counter.byte_range();

    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    placement.extend_image(
        &image,
        &mut after,
        source.table.schema().definition_page(),
        budget,
        cancelled,
    )?;
    placement.apply(&image, &records, &mut after, budget, cancelled)?;
    budget
        .charge_allocation((2 * PAGE_BYTES) as u64, 1)
        .map_err(InspectError::from)?;
    after
        .try_reserve_exact(2 * PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, (3 * PAGE_BYTES + size_of::<u32>()) as u64)
        .map_err(InspectError::from)?;
    after.extend_from_slice(&promotion.right);
    after.extend_from_slice(&promotion.left);
    after[root..root + PAGE_BYTES].copy_from_slice(&promotion.root);
    after[counter_range].copy_from_slice(&next.to_le_bytes());
    authority.apply_after_data(&image, &mut after, budget, cancelled)?;
    hint.apply_after_data(&image, &mut after, budget, cancelled)?;
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    drop(facts);
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: requests.len(),
    };
    proof::prove(&plan, scope, table, requests, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

#[cfg(test)]
mod tests;
