//! Growth candidates never pass through the equal-length journalable planner.
use super::super::{Ace16WritePreview, EditError, WriteBlocker, append, gates, preservation};
use super::{authority::Operation, placement::Plan};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, check_table, check_table_constraints,
    format::page::PAGE_BYTES, header::check, inspect_catalog, limits::MAX_IMAGE_BYTES,
    preservation::COPY_CHUNK_BYTES, table::indexes::IndexDataPatch,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn finish(
    image: NativeImage,
    operation: &Operation<'_>,
    plan: &Plan,
    indexes: &[IndexDataPatch],
    changed: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    qualify(&image, plan)?;
    let scope = image.catalog().catalog().scope();
    let original = check_table(
        &mut Cursor::new(image.as_bytes()),
        scope,
        operation.table,
        budget,
        cancelled,
    )?;
    let verified = operation.prove(&image, &original, budget, cancelled)?;
    verified.compare(plan, changed, budget, cancelled)?;
    super::audit::pages(plan.view(), budget, cancelled)?;
    super::map_audit::prove(
        &image,
        original.schema(),
        plan,
        &verified.records,
        budget,
        cancelled,
    )?;

    compose(
        image,
        operation.table,
        plan,
        indexes,
        changed,
        budget,
        cancelled,
    )
}

/// Complete-byte composition after an operation's independent record/map proof.
pub(in crate::table::mutation) fn compose(
    image: NativeImage,
    table: &str,
    plan: &Plan,
    indexes: &[IndexDataPatch],
    changed: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let after = assemble(&image, plan, indexes, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
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

/// Exact byte composition; each caller performs its own final table admission.
/// Existing ordinary finish and preserved Text finish are the only consumers.
pub(in crate::table::mutation) fn assemble(
    image: &NativeImage,
    plan: &Plan,
    indexes: &[IndexDataPatch],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    qualify(image, plan)?;

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
    let span_count = plan.pages.len() + plan.append.map_edits.len() + indexes.len();
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
    if next_eof != plan.append.length {
        return Err(EditError::Preservation);
    }
    for edit in &plan.append.map_edits {
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
    append::audit_bytes(
        &plan.append.map_edits,
        image.as_bytes(),
        &after,
        budget,
        cancelled,
    )?;
    Ok(after)
}

fn qualify(image: &NativeImage, plan: &Plan) -> Result<(), EditError> {
    if plan.append.original_length != image.as_bytes().len()
        || plan.append.length < plan.append.original_length
        || plan.append.length as u64 > MAX_IMAGE_BYTES
        || !plan.append.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }

    Ok(())
}
