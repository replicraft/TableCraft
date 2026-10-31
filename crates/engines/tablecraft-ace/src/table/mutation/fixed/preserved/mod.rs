//! Source-certified mixed-schema copies for fixed scalars and fitting Short Text.
mod dependencies;
mod int32;
mod namemap;
mod proof;
mod properties;
mod source;
mod stage;
mod text;

use super::FixedColumnUpdates;
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, WriteBlocker, header::check,
    inspect_table_schema, preservation::COPY_CHUNK_BYTES, table::mutation::patch::BytePatch,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn selected(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        let mut seen = false;
        for prior in &groups[..position] {
            name_work(prior.table(), group.table(), budget)?;
            if prior.table().eq_ignore_ascii_case(group.table()) {
                seen = true;
                break;
            }
        }
        if seen {
            continue;
        }
        let schema = inspect_table_schema(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            group.table(),
            budget,
            cancelled,
        )?;
        if projected(&schema, budget, cancelled)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn preview(
    image: NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    let first = groups.first().ok_or(EditError::Preservation)?;
    let mut assignments = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        name_work(first.table(), group.table(), budget)?;
        if !first.table().eq_ignore_ascii_case(group.table()) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        assignments = assignments
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
    }
    let qualified = source::capture(&image, first.table(), budget, cancelled)?;
    let (patches, changed) = stage::prepare(&image, &qualified, groups, budget, cancelled)?;
    let mut after = copy(image.as_bytes(), budget, cancelled)?;
    for patch in patches {
        check(budget, cancelled)?;
        let end = patch
            .offset()
            .checked_add(patch.before().len())
            .ok_or(EditError::Preservation)?;
        let bytes = after
            .get_mut(patch.offset()..end)
            .ok_or(EditError::Preservation)?;
        if bytes != patch.before() {
            return Err(EditError::Preservation);
        }
        bytes.copy_from_slice(patch.after());
    }

    // The proof receives original requests, never staged patches or normalized
    // values. It recomputes all permitted bits and compares the complete image.
    proof::prove(&image, groups, &after, changed, budget, cancelled)?;
    let candidate = image.recapture_fixed_candidate(&after, budget, cancelled)?;
    source::capture(&candidate, first.table(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        changed,
    ))
}

fn copy(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in bytes.chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        result.extend_from_slice(chunk);
    }
    Ok(result)
}

fn name_work(left: &str, right: &str, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, left.len().max(right.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(())
}

fn unique(
    references: &mut Vec<u32>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let levels = references.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = references
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    references.sort_unstable();
    references.dedup();
    check(budget, cancelled)?;
    Ok(references.len())
}

pub(super) fn projected(
    schema: &crate::TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for field in schema.columns() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        if field.scalar_type().is_none() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod larger_capture_tests;

#[cfg(test)]
mod short_text_sources;
