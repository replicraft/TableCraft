//! One atomic image append below the checked catalog/query ownership certificate.
use super::source::CheckedStorage;
use crate::{
    InspectError, NativeImage, QueryCreationError, TableSchema,
    format::{data_append::Page, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::MAX_SLOTS},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn build(
    image: &NativeImage,
    storage: &CheckedStorage,
    catalog: &[u8],
    fragments: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, QueryCreationError> {
    check(budget, cancelled)?;
    let length = image.as_bytes().len();
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(image.as_bytes());

    append(
        &mut after,
        &storage.catalog,
        storage.catalog_page,
        std::iter::once(catalog),
        budget,
        cancelled,
    )?;
    append(
        &mut after,
        &storage.queries,
        storage.query_page,
        fragments.iter().map(Vec::as_slice),
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(after)
}

fn append<'record>(
    after: &mut [u8],
    schema: &TableSchema,
    page_number: u32,
    records: impl Iterator<Item = &'record [u8]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    let page = page_mut(after, page_number)?;
    let mut framing = Page::capture_retained(page, schema.definition_page(), budget, cancelled)?;
    let mut added = 0_u32;
    for record in records {
        check(budget, cancelled)?;
        if framing.count >= MAX_SLOTS {
            return Err(QueryCreationError::Source);
        }
        Page::charge_append(record.len(), budget, cancelled)?;
        let range = framing.append(page, record.len())?;
        page.get_mut(range)
            .ok_or(QueryCreationError::Preservation)?
            .copy_from_slice(record);
        added = added.checked_add(1).ok_or(InspectError::StructureLimit)?;
    }
    let count = schema
        .declared_rows()
        .checked_add(added)
        .ok_or(InspectError::StructureLimit)?;
    let definition = page_mut(after, schema.definition_page())?;
    definition
        .get_mut(RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + size_of::<u32>())
        .ok_or(QueryCreationError::Preservation)?
        .copy_from_slice(&count.to_le_bytes());
    Ok(())
}

fn page_mut(bytes: &mut [u8], number: u32) -> Result<&mut [u8], QueryCreationError> {
    let start = usize::try_from(number)
        .ok()
        .and_then(|number| number.checked_mul(PAGE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    bytes
        .get_mut(start..end)
        .ok_or(QueryCreationError::Preservation)
}
