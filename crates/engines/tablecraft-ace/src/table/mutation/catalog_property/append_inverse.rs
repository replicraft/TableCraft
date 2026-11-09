//! Recover a bounded fitted-append hypothesis, never source or write authority.
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::u32_at,
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::row_range,
        row::{
            OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::mutation::owned_lval::placement::geometry,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_DEFINITION: usize = 2;
const CATALOG_PAGE: u32 = 4;
const PROPERTY_PAGE: u32 = 10;
const FREE_OFFSET: usize = 2;
const COPY_BYTES: usize = 16 * 1024;

/// Actual append locations and framed widths supplied by the operation owner.
/// These observations grant no permission to drop a catalog role or body.
pub(crate) struct LastRole {
    pub root: u32,
    pub property: u32,
    pub record_bytes: usize,
    pub body_bytes: usize,
}

pub(crate) fn hypothesis(
    image: &NativeImage,
    role: LastRole,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    if reference_page(role.root) != CATALOG_PAGE || reference_page(role.property) != PROPERTY_PAGE {
        return Err(EditError::Preservation);
    }
    let definition = image
        .as_bytes()
        .get(CATALOG_DEFINITION * PAGE_BYTES..(CATALOG_DEFINITION + 1) * PAGE_BYTES)
        .ok_or(InspectError::Malformed)?;
    let logical_count = u32_at(definition, RECORD_COUNT_OFFSET)?;
    if usize::try_from(logical_count).ok() != Some(image.catalog().origins().len()) {
        return Err(EditError::Preservation);
    }
    let previous = logical_count
        .checked_sub(1)
        .ok_or(EditError::Preservation)?;
    let length = image.as_bytes().len();
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        result.extend_from_slice(chunk);
    }
    for (reference, bytes) in [
        (role.root, role.record_bytes),
        (role.property, role.body_bytes),
    ] {
        check(budget, cancelled)?;
        let page = reference_page(reference) as usize;
        let data = result
            .get_mut(page * PAGE_BYTES..(page + 1) * PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        undo(data, usize::from(reference_slot(reference)), bytes, budget)?;
    }
    budget
        .charge(Resource::Steps, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let count = CATALOG_DEFINITION * PAGE_BYTES + RECORD_COUNT_OFFSET;
    result[count..count + size_of::<u32>()].copy_from_slice(&previous.to_le_bytes());
    check(budget, cancelled)?;
    Ok(result)
}

fn undo(
    page: &mut [u8],
    slot: usize,
    bytes: usize,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    let (count, low) = geometry(page)?;
    if slot == 0 || count.checked_sub(1) != Some(slot) {
        return Err(EditError::Preservation);
    }
    let (word, start, end) = row_range(page, slot)?;
    if word & !OFFSET_MASK != 0 || start != low || end - start != bytes {
        return Err(EditError::Preservation);
    }
    let previous_free = end
        .checked_sub(slot_offset(slot))
        .and_then(|free| u16::try_from(free).ok())
        .ok_or(EditError::Preservation)?;
    let previous_count = u16::try_from(slot).map_err(|_| InspectError::Malformed)?;
    // Only the lost append preimage is hypothesized as zero. The caller MUST
    // qualify the complete recovered source and replay its whole current image.
    page[start..end].fill(0);
    page[slot_offset(slot)..slot_offset(slot) + SLOT_BYTES].fill(0);
    page[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()]
        .copy_from_slice(&previous_count.to_le_bytes());
    page[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(&previous_free.to_le_bytes());
    Ok(())
}
