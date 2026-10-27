//! Growth candidates never pass through the equal-length journalable planner.
use super::super::{Ace16WritePreview, EditError, WriteBlocker, gates, preservation};
use super::placement::Plan;
use crate::{
    IndexCheckStatus, InspectError, NativeImage, check_table, check_table_constraints,
    format::page::PAGE_BYTES, header::check, inspect_catalog, limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES, table::indexes::IndexDataPatch,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn finish(
    image: NativeImage,
    table: &str,
    plan: &Plan,
    indexes: &[IndexDataPatch],
    changed: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    if plan.original_length != image.as_bytes().len()
        || plan.length < plan.original_length
        || plan.length as u64 > MAX_IMAGE_BYTES
        || !plan.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }

    let scope = image.catalog().catalog().scope();
    let original = check_table(
        &mut Cursor::new(image.as_bytes()),
        scope,
        table,
        budget,
        cancelled,
    )?;
    super::map_audit::prove(&image, original.schema(), plan, budget, cancelled)?;

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
    let span_count = plan.pages.len() + plan.maps.edits.len() + indexes.len();
    budget
        .charge_allocation(span_count as u64, size_of::<Range<usize>>() as u64)
        .map_err(InspectError::from)?;
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(span_count)
        .map_err(|_| InspectError::Allocation)?;
    let mut next_eof = image.as_bytes().len();

    for page in &plan.pages {
        check(budget, cancelled)?;
        let start = page.number as usize * PAGE_BYTES;
        let end = start
            .checked_add(page.after.len())
            .ok_or(EditError::Preservation)?;
        if page.before.is_empty() {
            if start != next_eof || page.after.len() != PAGE_BYTES {
                return Err(EditError::Preservation);
            }
            next_eof = end;
        } else {
            if image.as_bytes().get(start..end) != Some(page.before.as_slice()) {
                return Err(EditError::Preservation);
            }
            spans.push(start..end);
        }
        after
            .get_mut(start..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&page.after);
    }
    if next_eof != plan.length {
        return Err(EditError::Preservation);
    }
    for edit in &plan.maps.edits {
        check(budget, cancelled)?;
        if image.as_bytes().get(edit.offset) != Some(&edit.before)
            || after.get(edit.offset) != Some(&edit.before)
        {
            return Err(EditError::Preservation);
        }
        after[edit.offset] = edit.after;
        spans.push(edit.offset..edit.offset + 1);
    }
    for patch in indexes {
        check(budget, cancelled)?;
        let end = patch
            .offset
            .checked_add(patch.before.len())
            .ok_or(EditError::Preservation)?;
        if patch.before.len() != patch.after.len()
            || image.as_bytes().get(patch.offset..end) != Some(patch.before.as_slice())
            || after.get(patch.offset..end) != Some(patch.before.as_slice())
        {
            return Err(EditError::Preservation);
        }
        after[patch.offset..end].copy_from_slice(&patch.after);
        spans.push(patch.offset..end);
    }
    // This proof only sees the original prefix. Appended bytes are separately
    // certified complete initialized pages with no implicit zero tail.
    preservation::prove(
        image.as_bytes(),
        &after[..image.as_bytes().len()],
        spans,
        budget,
        cancelled,
    )?;
    for page in &plan.pages {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, page.after.len() as u64)
            .map_err(InspectError::from)?;
        let start = page.number as usize * PAGE_BYTES;
        if after.get(start..start + page.after.len()) != Some(page.after.as_slice()) {
            return Err(EditError::Preservation);
        }
    }
    plan.maps
        .audit_bytes(image.as_bytes(), &after, budget, cancelled)?;
    // Reparse the catalog/header from actual grown bytes; old page_count must
    // never constrain or authorize reads of the newly appended pages.
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
    plan.maps
        .audit_membership(&after, checked.schema(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        changed,
    ))
}
