//! Exact existing-page retirement, with an independent after-image map audit.
mod audit;
mod maps;
pub(in crate::table) use audit::audit_retirements;
pub(super) use maps::patches;

use super::super::header::IndexHeader;
use super::IndexDataPatch;
use crate::{EditError, InspectError, NativeImage, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) use crate::format::allocation::GLOBAL_FREE_REFERENCE as GLOBAL_MAP_REFERENCE;

pub(in crate::table) struct RetiredPage {
    pub number: u32,
    pub physical: u32,
    pub usage: u32,
    pub root: u32,
}

pub(super) const FIRST_REUSABLE_PAGE: u32 = 2;
pub(super) const INVALID_PREFIX_BYTES: usize = size_of::<u32>();

pub(super) fn header_patch(
    image: &NativeImage,
    page: u32,
    field: usize,
    after: &[u8],
    patches: &mut Vec<IndexDataPatch>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let offset = (page as usize)
        .checked_mul(PAGE_BYTES)
        .and_then(|offset| offset.checked_add(field))
        .ok_or(EditError::Preservation)?;
    let end = offset
        .checked_add(after.len())
        .ok_or(EditError::Preservation)?;
    let before = image
        .bytes
        .get(offset..end)
        .ok_or(EditError::Preservation)?;
    append(patches, offset, before, after, budget, cancelled)
}

pub(super) fn append(
    patches: &mut Vec<IndexDataPatch>,
    offset: usize,
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if before.len() != after.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    if before == after {
        return Ok(());
    }
    budget
        .charge_allocation((before.len() + after.len()) as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    let mut old = Vec::new();
    let mut new = Vec::new();
    old.try_reserve_exact(before.len())
        .map_err(|_| InspectError::Allocation)?;
    new.try_reserve_exact(after.len())
        .map_err(|_| InspectError::Allocation)?;
    old.extend_from_slice(before);
    new.extend_from_slice(after);
    patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    patches.push(IndexDataPatch {
        offset,
        before: old,
        after: new,
    });
    Ok(())
}

pub(super) fn leaf_links(
    image: &NativeImage,
    leaves: &[u32],
    patches: &mut Vec<IndexDataPatch>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for (position, &page) in leaves.iter().enumerate() {
        let previous = position.checked_sub(1).map_or(0, |index| leaves[index]);
        let next = leaves.get(position + 1).copied().unwrap_or(0);
        header_patch(
            image,
            page,
            std::mem::offset_of!(IndexHeader, previous),
            &previous.to_le_bytes(),
            patches,
            budget,
            cancelled,
        )?;
        header_patch(
            image,
            page,
            std::mem::offset_of!(IndexHeader, next),
            &next.to_le_bytes(),
            patches,
            budget,
            cancelled,
        )?;
    }
    Ok(())
}
