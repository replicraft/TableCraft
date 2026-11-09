//! Exact page events and unchanged complements for same-root placement.
use super::{
    forward::{EventKind, FREE_OFFSET, POINTER_BYTES, Plan},
    slots,
};
use crate::{
    EditError, InspectError,
    format::{
        bytes::{u16_at, u32_at},
        page::PAGE_BYTES,
        reader::row_count,
        row::{
            DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference,
            reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) fn pages(
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.cross_page {
        #[cfg(feature = "journal")]
        return super::owned_audit::pages(plan, budget, cancelled);
        #[cfg(not(feature = "journal"))]
        return Err(EditError::Preservation);
    }
    for page in &plan.pages {
        check(budget, cancelled)?;
        let count = row_count(&page.before)?;
        let mut end = slots::frontier(&page.before, budget, cancelled)?;
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
            if event.kind != EventKind::SamePage {
                return Err(EditError::Preservation);
            }
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
            // Pass bounds to scope admission, then reuse them to check exact existing-record
            // membership. Only range bounds are copied, not the payload.
            allow(target.clone())?;
            if event.appended.is_none() {
                let mut existing = false;
                for slot in 0..count {
                    check(budget, cancelled)?;
                    let (_, start, end) = slots::range(&page.before, slot)?;
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
                let (word, original_start, original_end) = slots::range(&page.before, root_slot)?;
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
            let (_, start, stop) = slots::range(&page.after, slot)?;
            let (_, old_start, old_stop) = slots::range(&page.before, slot)?;
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

pub(in crate::table::mutation) fn final_pages(
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
