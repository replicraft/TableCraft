//! Independent framing, unselected-field and final page authorization checks.
use super::super::{EditError, SlotScalarUpdate};
use super::{
    forward::{FREE_OFFSET, POINTER_BYTES, Plan},
    roots::Root,
};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        page::PAGE_BYTES,
        reader::{Reader, row_count, row_range},
        row::{
            DELETED_MASK, MASK_BITS, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES,
            reference, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::{record, record_layout},
};
use std::ops::Range;
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

pub(super) fn pages(
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for page in &plan.pages {
        check(budget, cancelled)?;
        let count = row_count(&page.before)?;
        let mut end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(&page.before, count - 1)?.1
        };
        let mut free = usize::from(u16_at(&page.before, FREE_OFFSET)?);
        if free != end - slot_offset(count) {
            return Err(EditError::Preservation);
        }
        let mut appended = 0;
        budget
            .charge_allocation(PAGE_BYTES as u64, size_of::<bool>() as u64)
            .map_err(InspectError::from)?;
        let mut allowed = Vec::new();
        allowed
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        allowed.resize(PAGE_BYTES, false);
        let base = page.number as usize * PAGE_BYTES;
        let mut allow = |range: Range<usize>| -> Result<(), EditError> {
            allowed
                .get_mut(range)
                .ok_or(EditError::Preservation)?
                .fill(true);
            Ok(())
        };
        for event in &page.events {
            check(budget, cancelled)?;
            let target = event
                .target
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?
                ..event
                    .target
                    .end
                    .checked_sub(base)
                    .ok_or(EditError::Preservation)?;
            allow(target.clone())?;
            if event.appended.is_none() {
                let mut existing = false;
                for slot in 0..count {
                    check(budget, cancelled)?;
                    let (_, start, end) = row_range(&page.before, slot)?;
                    existing |= target == (start..end);
                }
                if !existing {
                    return Err(EditError::Preservation);
                }
            }
            if let Some(slot) = event.appended {
                if usize::from(slot) != count + appended {
                    return Err(EditError::Preservation);
                }
                let length = target.len();
                end = end.checked_sub(length).ok_or(EditError::Preservation)?;
                free = free
                    .checked_sub(length + SLOT_BYTES)
                    .ok_or(EditError::Preservation)?;
                let target_word =
                    u16::try_from(end).map_err(|_| EditError::Preservation)? | DELETED_MASK;
                if target != (end..end + length)
                    || u16_at(&page.after, slot_offset(usize::from(slot)))? != target_word
                {
                    return Err(EditError::Preservation);
                }

                let root_slot = usize::from(reference_slot(event.root));
                let (word, original_start, original_end) = row_range(&page.before, root_slot)?;
                if reference_page(event.root) != page.number
                    || word != event.root_word
                    || event.root_range != (base + original_start..base + original_end)
                {
                    return Err(EditError::Preservation);
                }

                let root_start = event
                    .root_range
                    .start
                    .checked_sub(base)
                    .ok_or(EditError::Preservation)?;
                let root_end = event
                    .root_range
                    .end
                    .checked_sub(base)
                    .ok_or(EditError::Preservation)?;
                if root_end - root_start < POINTER_BYTES
                    || u32_at(&page.after, root_start)? != reference(page.number, slot)
                {
                    return Err(EditError::Preservation);
                }
                let word_at = slot_offset(usize::from(reference_slot(event.root)));
                let root_word = event.root_word | RELOCATED_MASK;
                if u16_at(&page.after, word_at)? != root_word {
                    return Err(EditError::Preservation);
                }
                if event.root_word & RELOCATED_MASK == 0 {
                    if page.after[root_start + POINTER_BYTES..root_end]
                        .iter()
                        .any(|byte| *byte != 0)
                    {
                        return Err(EditError::Preservation);
                    }
                    allow(root_start..root_end)?;
                } else {
                    allow(root_start..root_start + POINTER_BYTES)?;
                }
                allow(word_at..word_at + SLOT_BYTES)?;
                allow(slot_offset(usize::from(slot))..slot_offset(usize::from(slot) + 1))?;
                appended += 1;
            }
        }
        if appended != 0 {
            allow(FREE_OFFSET..FREE_OFFSET + size_of::<u16>())?;
            allow(ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>())?;
        }
        if row_count(&page.after)? != count + appended
            || usize::from(u16_at(&page.after, FREE_OFFSET)?) != free
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, (count * page.events.len()) as u64)
            .map_err(InspectError::from)?;
        for slot in 0..count {
            check(budget, cancelled)?;
            let old = u16_at(&page.before, slot_offset(slot))?;
            let promoted = page.events.iter().any(|event| {
                event.appended.is_some() && usize::from(reference_slot(event.root)) == slot
            });
            let expected = if promoted { old | RELOCATED_MASK } else { old };
            if u16_at(&page.after, slot_offset(slot))? != expected {
                return Err(EditError::Preservation);
            }
            let (_, start, stop) = row_range(&page.after, slot)?;
            let (_, old_start, old_stop) = row_range(&page.before, slot)?;
            if start != old_start || stop != old_stop || expected & OFFSET_MASK != old & OFFSET_MASK
            {
                return Err(EditError::Preservation);
            }
        }
        for (position, permitted) in allowed.iter().enumerate() {
            check(budget, cancelled)?;
            if !*permitted && page.before[position] != page.after[position] {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}

pub(super) fn final_pages(
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for page in &plan.pages {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        let start = page.number as usize * PAGE_BYTES;
        if after.get(start..start + PAGE_BYTES) != Some(page.after.as_slice()) {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
