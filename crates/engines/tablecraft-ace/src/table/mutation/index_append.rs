//! Exact-before composition shared by fixed and slot index-split candidates.
use super::{
    EditError, patch::BytePatch, preservation, slot::stage::Patch,
    variable_update::placement::DataView,
};
use crate::{
    InspectError, NativeImage, format::page::PAGE_BYTES, header::check, limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES, table::indexes::SplitIndexPlan,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn qualify(image: &NativeImage, plan: &SplitIndexPlan) -> Result<(), EditError> {
    qualify_after_data(image, plan, image.as_bytes().len())
}

fn qualify_after_data(
    image: &NativeImage,
    plan: &SplitIndexPlan,
    data_end: usize,
) -> Result<(), EditError> {
    if plan.append.original_length != image.as_bytes().len()
        || plan.append.length < plan.append.original_length
        || plan.append.length as u64 > MAX_IMAGE_BYTES
        || !plan.append.length.is_multiple_of(PAGE_BYTES)
        || data_end < plan.append.original_length
        || !data_end.is_multiple_of(PAGE_BYTES)
        || plan.added.len().checked_mul(PAGE_BYTES) != plan.append.length.checked_sub(data_end)
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
    compose_parts(image, fields, plan, None, budget, cancelled)
}

pub(super) fn compose_data(
    image: &NativeImage,
    data: DataView<'_>,
    plan: &SplitIndexPlan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    compose_parts::<Patch>(image, &[], plan, Some(data), budget, cancelled)
}

fn compose_parts<P: BytePatch>(
    image: &NativeImage,
    fields: &[P],
    plan: &SplitIndexPlan,
    data: Option<DataView<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let index_begin = data.map_or(image.as_bytes().len(), |data| data.length);
    qualify_after_data(image, plan, index_begin)?;
    budget
        .charge_allocation(plan.append.length as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(plan.append.length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (plan.append.length - after.len()) as u64)
        .map_err(InspectError::from)?;
    after.resize(plan.append.length, 0);
    let count = fields.len()
        + data.map_or(0, |data| data.pages.len())
        + plan.patches.len()
        + plan.append.map_edits.len()
        + plan.promotions.len();
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
    if let Some(data) = data {
        let mut next = image.as_bytes().len();
        for page in data.pages {
            check(budget, cancelled)?;
            let offset = page.number as usize * PAGE_BYTES;
            if page.after.len() != PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            if page.before.is_empty() {
                if offset != next || next + PAGE_BYTES > index_begin {
                    return Err(EditError::Preservation);
                }
                plan.append.require_data(page.number, budget, cancelled)?;
                budget
                    .charge(Resource::Steps, PAGE_BYTES as u64)
                    .map_err(InspectError::from)?;
                after[next..next + PAGE_BYTES].copy_from_slice(&page.after);
                next += PAGE_BYTES;
            } else {
                spans.push(write(
                    image,
                    &mut after,
                    offset,
                    &page.before,
                    &page.after,
                    budget,
                    cancelled,
                )?);
            }
        }
        if next != index_begin {
            return Err(EditError::Preservation);
        }
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
    for edit in &plan.append.map_edits {
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
        let start = index_begin + position * PAGE_BYTES;
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
        &after[..plan.append.original_length],
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
