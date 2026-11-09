//! All memberships, histories and original map authority precede candidate bytes.
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_index_counter,
    table::{
        indexes::mutation::insertion::{self, InsertRow, coordinated},
        mutation::{fixed_records, preserved_source::Source, record_insertion::properties},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation::normal_insert) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let facts = super::admission::qualify(&image, &source, table, requests, budget, cancelled)?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::super::records::qualify_complete(source.table.schema(), requests, budget, cancelled)?;
    let records =
        super::super::records::encode(source.table.schema(), requests, budget, cancelled)?;
    let placement = super::super::placement::reserve(&image, &source, &records, budget, cancelled)?;
    let mut rows = super::vector(requests.len(), budget, cancelled)?;
    for (reference, request) in placement.references().zip(requests) {
        check(budget, cancelled)?;
        rows.push(InsertRow {
            reference,
            values: request.values(),
        });
    }
    let prepared = coordinated::prepare(&image, facts.checked(), &rows, budget, cancelled)?;
    if prepared.primary.original_count() != source.records.len()
        || prepared.primary.expected().len() != source.records.len() + requests.len()
    {
        return Err(EditError::Preservation);
    }
    let indexes = [
        insertion::qualify_actions(facts.checked(), 1, budget, cancelled)?,
        insertion::qualify_actions(facts.checked(), 2, budget, cancelled)?,
    ];
    let authority = super::authority::Authority::capture(
        &image,
        source.table.schema(),
        indexes,
        budget,
        cancelled,
    )?;
    let mut histories = super::vector(3, budget, cancelled)?;
    for physical in 0..3_u32 {
        check(budget, cancelled)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        let original = inspect_table_index_counter(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            index.name(),
            budget,
            cancelled,
        )?;
        if original.definition_page() != source.table.schema().definition_page()
            || original.physical_index() != physical
            || prepared.distinct_added[physical as usize] > requests.len()
        {
            return Err(EditError::Preservation);
        }
        let delta = u32::try_from(prepared.distinct_added[physical as usize])
            .map_err(|_| InspectError::StructureLimit)?;
        let next = original
            .value()
            .checked_add(delta)
            .ok_or(InspectError::StructureLimit)?;
        histories.push((original.byte_range(), original.value(), next));
    }
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    placement.apply(&image, &records, &mut after, budget, cancelled)?;
    for patch in &prepared.primary.patches {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (patch.before.len() + patch.after.len()) as u64,
            )
            .map_err(InspectError::from)?;
        let end = patch
            .offset
            .checked_add(patch.before.len())
            .ok_or(EditError::Preservation)?;
        if patch.before.len() != patch.after.len()
            || after.get(patch.offset..end) != Some(patch.before.as_slice())
        {
            return Err(EditError::Preservation);
        }
        after[patch.offset..end].copy_from_slice(&patch.after);
    }
    super::reserve_pages(&mut after, 4, budget, cancelled)?;
    for (position, promotion) in prepared.wide.iter().enumerate() {
        check(budget, cancelled)?;
        let root = indexes[position].root_page() as usize * PAGE_BYTES;
        budget
            .charge(Resource::Steps, (3 * PAGE_BYTES) as u64)
            .map_err(InspectError::from)?;
        after[root..root + PAGE_BYTES].copy_from_slice(&promotion.root);
        after.extend_from_slice(&promotion.right);
        after.extend_from_slice(&promotion.left);
    }
    for (range, old, next) in histories {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * size_of::<u32>()) as u64)
            .map_err(InspectError::from)?;
        // NO_CLONE: Check old counter bytes before consuming the same bounds to write the
        // new counter. Only range bounds are copied, not the payload.
        if after.get(range.clone()) != Some(old.to_le_bytes().as_slice()) {
            return Err(EditError::Preservation);
        }
        after[range].copy_from_slice(&next.to_le_bytes());
    }
    authority.apply(&image, source.table.schema(), &mut after, budget, cancelled)?;
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    drop(facts);
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: requests.len(),
    };
    super::proof::prove(&plan, scope, table, requests, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}
