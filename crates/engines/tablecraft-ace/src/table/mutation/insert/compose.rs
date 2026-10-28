//! Exact-before composition; independent final proof owns insertion authority.
use super::Staged;
use crate::{
    EditError, InspectError, NativeImage,
    format::page::{PAGE_BYTES, USAGE_SIGNATURE},
    header::check,
    limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn compose(
    image: &NativeImage,
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let data = &staged.data;
    let original = image.as_bytes();
    if data.append.original_length != original.len()
        || data.append.length < original.len()
        || data.append.length as u64 > MAX_IMAGE_BYTES
        || !data.append.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }

    budget
        .charge_allocation(data.append.length as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(data.append.length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in original.chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    budget
        .charge(
            Resource::Steps,
            (data.append.length - original.len()) as u64,
        )
        .map_err(InspectError::from)?;
    after.resize(data.append.length, 0);

    let count = data
        .pages
        .len()
        .checked_add(data.append.map_edits.len())
        .and_then(|count| count.checked_add(staged.indexes.patches.len()))
        .and_then(|count| count.checked_add(staged.indexes.promotions.len()))
        .and_then(|count| count.checked_add(staged.counters.len()))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(count as u64, size_of::<Range<usize>>() as u64)
        .map_err(InspectError::from)?;
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for page in &data.pages {
        check(budget, cancelled)?;
        if !page.before.is_empty() {
            spans.push(write(
                original,
                &mut after,
                page.number as usize * PAGE_BYTES,
                &page.before,
                &page.after,
                budget,
                cancelled,
            )?);
        }
    }
    for start in (original.len()..after.len()).step_by(PAGE_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (data.pages.len() + staged.indexes.added.len() + PAGE_BYTES) as u64,
            )
            .map_err(InspectError::from)?;
        let number = u32::try_from(start / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
        let mut selected = None;
        for page in &data.pages {
            if page.number == number && page.before.is_empty() {
                data.append.require_data(number, budget, cancelled)?;
                if selected.replace(page.after.as_slice()).is_some() {
                    return Err(EditError::Preservation);
                }
            }
        }
        for (bitmap, reference, bytes) in data.maps.metadata_pages() {
            if bitmap == number {
                data.append.require_candidate_bitmap(
                    number,
                    data.append.owner,
                    reference,
                    budget,
                    cancelled,
                )?;
                if selected.replace(bytes).is_some() {
                    return Err(EditError::Preservation);
                }
            }
        }
        for page in &staged.indexes.added {
            if page.number != number {
                continue;
            }
            if page.bytes.starts_with(&USAGE_SIGNATURE) {
                data.append.require_index_bitmap(
                    number,
                    staged.indexes.physical(),
                    staged.indexes.usage(),
                    budget,
                    cancelled,
                )?;
            } else {
                budget
                    .charge(Resource::Steps, data.append.reservations_len() as u64)
                    .map_err(InspectError::from)?;
                if !data.append.indexes().any(|reservation| {
                    reservation.number == number
                        && reservation.physical == staged.indexes.physical()
                        && reservation.usage == staged.indexes.usage()
                }) {
                    return Err(EditError::Preservation);
                }
            }
            if selected.replace(page.bytes.as_slice()).is_some() {
                return Err(EditError::Preservation);
            }
        }
        let bytes = selected
            .filter(|bytes| bytes.len() == PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        after[start..start + PAGE_BYTES].copy_from_slice(bytes);
    }
    for patch in &staged.indexes.patches {
        spans.push(write(
            original,
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?);
    }
    for patch in &staged.counters {
        spans.push(write(
            original,
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?);
    }
    for promotion in &staged.indexes.promotions {
        spans.push(write(
            original,
            &mut after,
            promotion.offset,
            &promotion.before,
            &promotion.after,
            budget,
            cancelled,
        )?);
    }
    for edit in &data.append.map_edits {
        spans.push(write(
            original,
            &mut after,
            edit.offset,
            &[edit.before],
            &[edit.after],
            budget,
            cancelled,
        )?);
    }

    let levels = spans.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = spans
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    spans.sort_unstable_by_key(|range| range.start);
    for pair in spans.windows(2) {
        check(budget, cancelled)?;
        if pair[0].end > pair[1].start {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(after)
}

fn write(
    original: &[u8],
    after: &mut [u8],
    start: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Range<usize>, EditError> {
    check(budget, cancelled)?;
    if before.is_empty() || before.len() != replacement.len() {
        return Err(EditError::Preservation);
    }
    let end = start
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    if original.get(start..end) != Some(before) {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    after
        .get_mut(start..end)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(replacement);
    Ok(start..end)
}
