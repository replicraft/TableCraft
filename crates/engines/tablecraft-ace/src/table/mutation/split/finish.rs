//! Append-aware split certification returns no journalable edit plan.
use super::super::{Ace16WritePreview, EditError, WriteBlocker, gates, preservation};
use super::{Operation, Staged};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, check_table, check_table_constraints,
    format::page::PAGE_BYTES, header::check, inspect_catalog, limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn finish(
    image: NativeImage,
    operation: &Operation<'_>,
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let table = operation.table;
    let structural = &staged.structural;
    let fields = &staged.patches;
    let plan = &staged.plan;
    check(budget, cancelled)?;
    if plan.original_length != image.as_bytes().len()
        || plan.length < plan.original_length
        || plan.length as u64 > MAX_IMAGE_BYTES
        || !plan.length.is_multiple_of(PAGE_BYTES)
        || plan.length - plan.original_length != plan.added.len() * PAGE_BYTES
    {
        return Err(EditError::Preservation);
    }
    let keys = super::authority::prove(&image, operation, structural, fields, budget, cancelled)?;
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
    let count = fields.len() + plan.patches.len() + plan.map_edits.len();
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
            &image,
            &mut after,
            field.offset,
            &field.before,
            &field.after,
            budget,
            cancelled,
        )?);
    }
    for patch in &plan.patches {
        check(budget, cancelled)?;
        spans.push(write(
            &image,
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?);
    }
    for edit in &plan.map_edits {
        check(budget, cancelled)?;
        spans.push(write(
            &image,
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
    plan.audit(&image, structural, &keys, &after, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let checked = check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?;
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    gates::qualified(&check_table_constraints(
        &mut Cursor::new(&after),
        scope,
        table,
        budget,
        cancelled,
    )?)?;
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        fields.len(),
    ))
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
