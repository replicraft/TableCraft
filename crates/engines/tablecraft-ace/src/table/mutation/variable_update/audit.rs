//! Independent root/destination authorization and exact EOF framing.
#[cfg(test)]
mod tests;

use super::super::EditError;
use super::placement::DataView;
use crate::{
    InspectError,
    format::{
        bytes::u16_at,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{
            DELETED_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, reference, reference_page,
            reference_slot, slot_offset,
        },
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const POINTER_BYTES: usize = size_of::<u32>();

pub(in crate::table::mutation) fn pages(
    plan: DataView<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut added = 0;
    for page in plan.pages {
        check(budget, cancelled)?;
        budget
            .charge_allocation((2 * PAGE_BYTES) as u64, 1)
            .map_err(InspectError::from)?;
        let mut initial = Vec::new();
        initial
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        if page.before.is_empty() {
            plan.append.require_data(page.number, budget, cancelled)?;
            added += 1;
            if page.number as usize * PAGE_BYTES < plan.append.original_length {
                return Err(EditError::Preservation);
            }
            initial.resize(PAGE_BYTES, 0);
            initial[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
            let free = PAGE_BYTES - slot_offset(0);
            initial[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                .copy_from_slice(&(free as u16).to_le_bytes());
            initial[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
                .copy_from_slice(&plan.append.owner.to_le_bytes());
        } else {
            if page.before.len() != PAGE_BYTES
                || page.number as usize * PAGE_BYTES >= plan.append.original_length
            {
                return Err(EditError::Preservation);
            }
            initial.extend_from_slice(&page.before);
        }
        let old_count = row_count(&initial)?;
        let mut count = old_count;
        let mut end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(&initial, count - 1)?.1
        };
        if usize::from(u16_at(&initial, FREE_OFFSET)?) != end - slot_offset(count) {
            return Err(EditError::Preservation);
        }
        let mut allowed = Vec::new();
        allowed
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        allowed.resize(PAGE_BYTES, false);
        let base = page.number as usize * PAGE_BYTES;

        for event in plan.events {
            check(budget, cancelled)?;
            if event.target.start / PAGE_BYTES == page.number as usize {
                let start = event
                    .target
                    .start
                    .checked_sub(base)
                    .ok_or(EditError::Preservation)?;
                let stop = event
                    .target
                    .end
                    .checked_sub(base)
                    .ok_or(EditError::Preservation)?;
                allowed
                    .get_mut(start..stop)
                    .ok_or(EditError::Preservation)?
                    .fill(true);
                if let Some(slot) = event.appended {
                    if usize::from(slot) != count || stop != end {
                        return Err(EditError::Preservation);
                    }
                    end = end
                        .checked_sub(stop - start)
                        .ok_or(EditError::Preservation)?;
                    if start != end
                        || end < slot_offset(count + 1)
                        || u16_at(&page.after, slot_offset(count))?
                            != (u16::try_from(end).map_err(|_| EditError::Preservation)?
                                | DELETED_MASK)
                    {
                        return Err(EditError::Preservation);
                    }
                    allowed[slot_offset(count)..slot_offset(count + 1)].fill(true);
                    count += 1;
                } else {
                    let mut found = false;
                    for slot in 0..old_count {
                        check(budget, cancelled)?;
                        let (_, old_start, old_end) = row_range(&initial, slot)?;
                        found |= start == old_start && stop == old_end;
                    }
                    if !found {
                        return Err(EditError::Preservation);
                    }
                }
            }
            if event.appended.is_some() && reference_page(event.root) == page.number {
                let slot = usize::from(reference_slot(event.root));
                let (word, start, stop) = row_range(&initial, slot)?;
                if word != event.word
                    || event.root_range != (base + start..base + stop)
                    || stop - start < POINTER_BYTES
                {
                    return Err(EditError::Preservation);
                }
                let destination_page = u32::try_from(event.target.start / PAGE_BYTES)
                    .map_err(|_| EditError::Preservation)?;
                let destination = reference(
                    destination_page,
                    event.appended.ok_or(EditError::Preservation)?,
                );
                if page.after.get(start..start + POINTER_BYTES)
                    != Some(destination.to_le_bytes().as_slice())
                    || u16_at(&page.after, slot_offset(slot))? != word | RELOCATED_MASK
                {
                    return Err(EditError::Preservation);
                }
                budget
                    .charge(Resource::Steps, (stop - start) as u64)
                    .map_err(InspectError::from)?;
                let remainder = &page.after[start + POINTER_BYTES..stop];
                if word & RELOCATED_MASK == 0 {
                    if remainder.iter().any(|byte| *byte != 0) {
                        return Err(EditError::Preservation);
                    }
                } else if remainder != &initial[start + POINTER_BYTES..stop] {
                    return Err(EditError::Preservation);
                }
                allowed[start..stop].fill(true);
                allowed[slot_offset(slot)..slot_offset(slot + 1)].fill(true);
            }
        }
        if row_count(&page.after)? != count
            || usize::from(u16_at(&page.after, FREE_OFFSET)?) != end - slot_offset(count)
        {
            return Err(EditError::Preservation);
        }
        allowed[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].fill(true);
        allowed[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].fill(true);
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        for (position, permitted) in allowed.iter().enumerate() {
            check(budget, cancelled)?;
            if !permitted && initial[position] != page.after[position] {
                return Err(EditError::Preservation);
            }
        }
        // Original row boundaries remain fixed even if root flags changed.
        for slot in 0..old_count {
            check(budget, cancelled)?;
            let (_, old_start, old_end) = row_range(&initial, slot)?;
            let (_, new_start, new_end) = row_range(&page.after, slot)?;
            if old_start != new_start || old_end != new_end {
                return Err(EditError::Preservation);
            }
        }
    }
    if plan.length != plan.append.original_length + added * PAGE_BYTES {
        return Err(EditError::Preservation);
    }
    Ok(())
}
