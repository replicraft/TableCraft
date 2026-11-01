//! Original-derived records and index pages, published only after fresh proof.
use super::{admission, authority, proof, selection};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker,
    format::page::PAGE_BYTES,
    header::check,
    inspect_table_index_counter,
    table::{
        indexes::mutation::insertion::{self, InsertRow},
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
    let _span = tracing::debug_span!("native.ordinary.insert.root_split.plan").entered();
    if !selection::selected(&image, &source, requests)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let (facts, _) = admission::qualify(&image, &source, table, requests, budget, cancelled)?;
    let index = insertion::qualify(facts.checked(), 0, budget, cancelled)?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::super::records::qualify(source.table.schema(), requests, budget, cancelled)?;
    let records =
        super::super::records::encode(source.table.schema(), requests, budget, cancelled)?;
    let placement = super::super::placement::reserve(&image, &source, &records, budget, cancelled)?;
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
    let authority =
        authority::Authority::capture(&image, source.table.schema(), index, budget, cancelled)?;
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
        insertion::prepare_root_promotion(&image, facts.checked(), &rows, budget, cancelled)?;
    let root = index.root_page() as usize * PAGE_BYTES;
    let counter_range = counter.byte_range();

    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
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
    authority.apply(&image, &mut after, budget, cancelled)?;
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
