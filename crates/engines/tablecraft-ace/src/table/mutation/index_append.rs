//! Exact-before composition shared by fixed and slot index-split candidates.
use super::{EditError, patch::BytePatch, preservation};
use crate::{
    InspectError, NativeImage, format::page::PAGE_BYTES, header::check, limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES, table::indexes::SplitIndexPlan,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn qualify(image: &NativeImage, plan: &SplitIndexPlan) -> Result<(), EditError> {
    if plan.original_length != image.as_bytes().len()
        || plan.length < plan.original_length
        || plan.length as u64 > MAX_IMAGE_BYTES
        || !plan.length.is_multiple_of(PAGE_BYTES)
        || plan.added.len().checked_mul(PAGE_BYTES) != plan.length.checked_sub(plan.original_length)
    {
        return Err(EditError::Preservation);
    }
    Ok(())
}

pub(super) fn compose<P: BytePatch>(
    image: &NativeImage,
    fields: &[P],
    plan: &SplitIndexPlan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    qualify(image, plan)?;
    budget
        .charge_allocation(plan.length as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(plan.length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (plan.length - after.len()) as u64)
        .map_err(InspectError::from)?;
    after.resize(plan.length, 0);
    let count = fields.len() + plan.patches.len() + plan.map_edits.len() + plan.promotions.len();
    budget
        .charge_allocation(count as u64, size_of::<Range<usize>>() as u64)
        .map_err(InspectError::from)?;
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for field in fields {
        check(budget, cancelled)?;
        spans.push(write(
            image,
            &mut after,
            field.offset(),
            field.before(),
            field.after(),
            budget,
            cancelled,
        )?);
    }
    for patch in &plan.patches {
        check(budget, cancelled)?;
        spans.push(write(
            image,
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?);
    }
    for promotion in &plan.promotions {
        check(budget, cancelled)?;
        spans.push(write(
            image,
            &mut after,
            promotion.offset,
            &promotion.before,
            &promotion.after,
            budget,
            cancelled,
        )?);
    }
    for edit in &plan.map_edits {
        check(budget, cancelled)?;
        spans.push(write(
            image,
            &mut after,
            edit.offset,
            &[edit.before],
            &[edit.after],
            budget,
            cancelled,
        )?);
    }
    for (position, page) in plan.added.iter().enumerate() {
        check(budget, cancelled)?;
        let start = plan.original_length + position * PAGE_BYTES;
        if page.number as usize * PAGE_BYTES != start || page.bytes.len() != PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        after
            .get_mut(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&page.bytes);
    }
    preservation::prove(
        image.as_bytes(),
        &after[..plan.original_length],
        spans,
        budget,
        cancelled,
    )?;
    Ok(after)
}

fn write(
    image: &NativeImage,
    after: &mut [u8],
    offset: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Range<usize>, EditError> {
    check(budget, cancelled)?;
    if before.len() > PAGE_BYTES {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, (3 * before.len()) as u64)
        .map_err(InspectError::from)?;
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    if before.len() != replacement.len()
        || end > image.as_bytes().len()
        || image.as_bytes().get(offset..end) != Some(before)
        || after.get(offset..end) != Some(before)
    {
        return Err(EditError::Preservation);
    }
    after[offset..end].copy_from_slice(replacement);
    Ok(offset..end)
}
