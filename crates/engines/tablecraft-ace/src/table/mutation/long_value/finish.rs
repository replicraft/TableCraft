//! Composed growth candidates stay outside equal-length apply/journal plans.
use super::super::{Ace16WritePreview, EditError, WriteBlocker, gates, preservation};
use super::{Operation, audit, authority, map_audit, stage::Staged};
use crate::{
    IndexCheckStatus, InspectError, LongValueCheckStatus, NativeImage, check_table,
    check_table_constraints, check_table_long_values, format::page::PAGE_BYTES, header::check,
    limits::MAX_IMAGE_BYTES, preservation::COPY_CHUNK_BYTES,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn finish(
    image: NativeImage,
    operation: &Operation<'_>,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    let plan = &staged.plan;
    if plan.original_length != image.as_bytes().len()
        || plan.length < plan.original_length
        || plan.length as u64 > MAX_IMAGE_BYTES
        || !plan.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    let prepared = authority::prove(&image, operation, staged, budget, cancelled)?;
    budget
        .charge_allocation(plan.length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(plan.length)
        .map_err(|_| InspectError::Allocation)?;
    for chunk in image.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        bytes.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (plan.length - bytes.len()) as u64)
        .map_err(InspectError::from)?;
    bytes.resize(plan.length, 0);
    let spans_count = plan
        .pages
        .len()
        .checked_add(plan.maps.edits.len())
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(spans_count as u64, size_of::<Range<usize>>() as u64)
        .map_err(InspectError::from)?;
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(spans_count)
        .map_err(|_| InspectError::Allocation)?;
    let mut next_eof = image.as_bytes().len();
    for (position, page) in plan.pages.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if plan.pages[..position]
            .iter()
            .any(|earlier| earlier.number == page.number)
            || page.after.len() != PAGE_BYTES
        {
            return Err(EditError::Preservation);
        }
        let start = page.number as usize * PAGE_BYTES;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if page.before.is_empty() {
            if start != next_eof {
                return Err(EditError::Preservation);
            }
            next_eof = end;
        } else {
            if image.as_bytes().get(start..end) != Some(page.before.as_slice()) {
                return Err(EditError::Preservation);
            }
            spans.push(start..end);
        }
        bytes
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
            || bytes.get(edit.offset) != Some(&edit.before)
        {
            return Err(EditError::Preservation);
        }
        bytes[edit.offset] = edit.after;
        spans.push(edit.offset..edit.offset + 1);
    }
    preservation::prove(
        image.as_bytes(),
        &bytes[..image.as_bytes().len()],
        spans,
        budget,
        cancelled,
    )?;

    let scope = image.catalog().catalog().scope();
    let after = NativeImage::capture(&mut Cursor::new(&bytes), scope, budget, cancelled)?;
    let schema = prepared.structural.schema();
    let checked = check_table(&mut after.reader(), scope, schema.name(), budget, cancelled)?;
    if checked.schema().declared_rows() != schema.declared_rows()
        || checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    gates::qualified(&check_table_constraints(
        &mut after.reader(),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?)?;
    let long_values =
        check_table_long_values(&mut after.reader(), scope, schema.name(), budget, cancelled)?;
    let selected = long_values
        .columns()
        .iter()
        .find(|column| column.column_number() == schema.columns()[prepared.column].number())
        .ok_or(EditError::Preservation)?;
    if selected.status() != LongValueCheckStatus::Checked
        || selected.shared_references() != 0
        || selected.shared_owned_pages() != 0
        || long_values.rows() != prepared.storage.rows()
    {
        return Err(EditError::Preservation);
    }
    map_audit::prove(&image, &prepared, plan, &after, budget, cancelled)?;
    audit::prove(&image, &after, &prepared, staged, budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after.bytes,
        staged.changes.len(),
    ))
}
