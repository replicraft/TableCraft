//! Independent framing and unselected-field record authorization checks.
use super::super::{EditError, SlotScalarUpdate};
use super::roots::Root;
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{reader::Reader, row::MASK_BITS},
    header::check,
    table::{record, record_layout},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const AUDIT_CHUNK_BYTES: usize = 256;

pub(in crate::table::mutation) fn record(
    image: &NativeImage,
    schema: &TableSchema,
    column: usize,
    root: &Root<'_, SlotScalarUpdate>,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let before = &root.final_record.data;
    let old = record_layout::parse(before, schema, budget)?;
    let new = record_layout::parse(after, schema, budget)?;
    let selected = schema.columns.get(column).ok_or(EditError::Preservation)?;
    if old.count != new.count
        || old.offsets.len() != new.offsets.len()
        || selected.fixed()
        || selected.number() >= old.count
    {
        return Err(EditError::Preservation);
    }
    let variable = selected.field.variable;
    let old_start = *old.offsets.get(variable).ok_or(EditError::Preservation)?;
    let old_end = *old
        .offsets
        .get(variable + 1)
        .ok_or(EditError::Preservation)?;
    let new_start = *new.offsets.get(variable).ok_or(EditError::Preservation)?;
    let new_end = *new
        .offsets
        .get(variable + 1)
        .ok_or(EditError::Preservation)?;
    if old_start != new_start {
        return Err(EditError::Preservation);
    }

    // Every physical slot, including unrepresented historical payloads, keeps
    // its boundary relative to the selected span. No active-column list can
    // substitute for the complete native offset directory.
    for (position, original) in old.offsets.iter().copied().enumerate() {
        check(budget, cancelled)?;
        let expected = if position <= variable {
            original
        } else {
            original
                .checked_sub(old_end - old_start)
                .and_then(|offset| offset.checked_add(new_end - new_start))
                .ok_or(EditError::Preservation)?
        };
        if new.offsets[position] != expected {
            return Err(EditError::Preservation);
        }
    }
    let old_payload_end = *old.offsets.last().ok_or(EditError::Preservation)?;
    let new_payload_end = *new.offsets.last().ok_or(EditError::Preservation)?;
    let footer = before
        .len()
        .checked_sub(old.data_end)
        .ok_or(EditError::Preservation)?;
    let minimum = new_payload_end
        .checked_add(footer)
        .ok_or(EditError::Preservation)?;
    if after.len() != before.len().max(minimum) {
        return Err(EditError::Preservation);
    }

    // Prefix/suffix comparisons certify fixed gaps and all opaque variable
    // slots as well as declared values. Retained padding stays stationary.
    unchanged(
        before.get(..old_start).ok_or(EditError::Preservation)?,
        after.get(..new_start).ok_or(EditError::Preservation)?,
        budget,
        cancelled,
    )?;
    unchanged(
        before
            .get(old_end..old_payload_end)
            .ok_or(EditError::Preservation)?,
        after
            .get(new_end..new_payload_end)
            .ok_or(EditError::Preservation)?,
        budget,
        cancelled,
    )?;
    if new_payload_end < new.data_end {
        unchanged(
            before
                .get(new_payload_end..new.data_end)
                .ok_or(EditError::Preservation)?,
            after
                .get(new_payload_end..new.data_end)
                .ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
    }
    let old_mask = before
        .get(old.mask_start..)
        .ok_or(EditError::Preservation)?;
    let new_mask = after.get(new.mask_start..).ok_or(EditError::Preservation)?;
    if old_mask.len() != new_mask.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, old_mask.len() as u64)
        .map_err(InspectError::from)?;
    let selected_byte = selected.number() / MASK_BITS;
    let selected_bit = 1_u8 << (selected.number() % MASK_BITS);
    for (position, (&old, &new)) in old_mask.iter().zip(new_mask).enumerate() {
        check(budget, cancelled)?;
        let permitted = if position == selected_byte {
            selected_bit
        } else {
            0
        };
        if (old ^ new) & !permitted != 0 {
            return Err(EditError::Preservation);
        }
    }

    let mut source = image.reader();
    let values = record::decode_selected(
        after,
        schema,
        Some(&[column]),
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        },
    )?;
    if values.as_slice() != std::slice::from_ref(root.update.value()) {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn unchanged(
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
    for (left, right) in before
        .chunks(AUDIT_CHUNK_BYTES)
        .zip(after.chunks(AUDIT_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        if left != right {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
