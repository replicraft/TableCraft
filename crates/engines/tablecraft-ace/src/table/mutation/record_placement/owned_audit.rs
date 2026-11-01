//! Cross-page operations authorize separate target and logical-root changes.
use super::{
    forward::{Event, EventKind, FREE_OFFSET, POINTER_BYTES, Page, Plan},
    slots,
};
use crate::{
    EditError, InspectError,
    format::{
        bytes::{u16_at, u32_at},
        page::{PAGE_BYTES, PAGE_NUMBER_MASK},
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

pub(super) fn pages(
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    pairing(plan, budget, cancelled)?;
    for page in &plan.pages {
        check(budget, cancelled)?;
        let count = row_count(&page.before)?;
        let mut end = slots::frontier(&page.before, budget, cancelled)?;
        let mut free = usize::from(u16_at(&page.before, FREE_OFFSET)?);
        if end.checked_sub(slot_offset(count)) != Some(free) {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(PAGE_BYTES as u64, size_of::<bool>() as u64)
            .map_err(InspectError::from)?;
        let mut allowed = Vec::new();
        allowed
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        allowed.resize(PAGE_BYTES, false);
        let base = page.number as usize * PAGE_BYTES;
        let mut appended = 0_usize;
        for event in &page.events {
            check(budget, cancelled)?;
            if event.kind != EventKind::ForwardRoot {
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
                permit(&mut allowed, start..stop)?;
                if let Some(slot) = event.appended {
                    if usize::from(slot) != count + appended {
                        return Err(EditError::Preservation);
                    }
                    let length = stop.checked_sub(start).ok_or(EditError::Preservation)?;
                    if length == 0 {
                        return Err(EditError::Preservation);
                    }
                    end = end.checked_sub(length).ok_or(EditError::Preservation)?;
                    free = free
                        .checked_sub(length + SLOT_BYTES)
                        .ok_or(EditError::Preservation)?;
                    if start != end
                        || stop != end + length
                        || u16_at(&page.after, slot_offset(usize::from(slot)))?
                            != (u16::try_from(start).map_err(|_| EditError::Preservation)?
                                | DELETED_MASK)
                    {
                        return Err(EditError::Preservation);
                    }
                    permit(
                        &mut allowed,
                        slot_offset(usize::from(slot))..slot_offset(usize::from(slot) + 1),
                    )?;
                    appended += 1;
                } else {
                    if event.kind != EventKind::SamePage {
                        return Err(EditError::Preservation);
                    }
                    let mut found = false;
                    for slot in 0..count {
                        check(budget, cancelled)?;
                        let (_, before_start, before_end) = slots::range(&page.before, slot)?;
                        found |= (before_start, before_end) == (start, stop);
                    }
                    if !found {
                        return Err(EditError::Preservation);
                    }
                }
            }
            if event.appended.is_some() && event.kind != EventKind::AppendTarget {
                root(page, event, &mut allowed, budget, cancelled)?;
            }
        }
        if appended != 0 {
            permit(&mut allowed, FREE_OFFSET..FREE_OFFSET + size_of::<u16>())?;
            permit(
                &mut allowed,
                ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>(),
            )?;
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
                event.appended.is_some()
                    && event.kind != EventKind::AppendTarget
                    && reference_page(event.root) == page.number
                    && usize::from(reference_slot(event.root)) == slot
            });
            let expected = if promoted { old | RELOCATED_MASK } else { old };
            let (_, start, end) = slots::range(&page.before, slot)?;
            let (_, after_start, after_end) = slots::range(&page.after, slot)?;
            if u16_at(&page.after, slot_offset(slot))? != expected
                || (start, end) != (after_start, after_end)
            {
                return Err(EditError::Preservation);
            }
        }
        for (index, allowed) in allowed.into_iter().enumerate() {
            check(budget, cancelled)?;
            if !allowed && page.before[index] != page.after[index] {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}

fn root(
    page: &Page,
    event: &Event,
    allowed: &mut [bool],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let base = page.number as usize * PAGE_BYTES;
    let slot = usize::from(reference_slot(event.root));
    let (word, start, end) = slots::range(&page.before, slot)?;
    if reference_page(event.root) != page.number
        || word != event.root_word
        || word & !(OFFSET_MASK | RELOCATED_MASK) != 0
        || event.root_range != (base + start..base + end)
        || end - start < POINTER_BYTES
    {
        return Err(EditError::Preservation);
    }
    let target_page =
        u32::try_from(event.target.start / PAGE_BYTES).map_err(|_| EditError::Preservation)?;
    let target_slot = event.appended.ok_or(EditError::Preservation)?;
    if u32_at(&page.after, start)? != reference(target_page, target_slot)
        || u16_at(&page.after, slot_offset(slot))? != (word | RELOCATED_MASK)
    {
        return Err(EditError::Preservation);
    }
    if word & RELOCATED_MASK == 0 {
        budget
            .charge(Resource::Steps, (end - start) as u64)
            .map_err(InspectError::from)?;
        if page.after[start + POINTER_BYTES..end]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(EditError::Preservation);
        }
        permit(allowed, start..end)?;
    } else {
        permit(allowed, start..start + POINTER_BYTES)?;
    }
    permit(allowed, slot_offset(slot)..slot_offset(slot + 1))?;
    Ok(())
}

fn pairing(
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for page in &plan.pages {
        for event in &page.events {
            check(budget, cancelled)?;
            if event.kind == EventKind::SamePage {
                continue;
            }
            let target_page = u32::try_from(event.target.start / PAGE_BYTES)
                .map_err(|_| EditError::Preservation)?;
            if target_page & !PAGE_NUMBER_MASK != 0
                || target_page == reference_page(event.root)
                || event.target.is_empty()
                || (event.target.end - 1) / PAGE_BYTES != event.target.start / PAGE_BYTES
                || event.appended.is_none()
                || (event.kind == EventKind::AppendTarget && page.number != target_page)
                || (event.kind == EventKind::ForwardRoot
                    && page.number != reference_page(event.root))
            {
                return Err(EditError::Preservation);
            }
            let mut partners = 0_usize;
            for other_page in &plan.pages {
                for other in &other_page.events {
                    check(budget, cancelled)?;
                    budget
                        .charge(Resource::Steps, size_of::<Event>() as u64)
                        .map_err(InspectError::from)?;
                    if other.kind == event.kind || other.kind == EventKind::SamePage {
                        continue;
                    }
                    if other.root == event.root
                        && other.root_word == event.root_word
                        && other.root_range == event.root_range
                        && other.target == event.target
                        && other.appended == event.appended
                    {
                        partners += 1;
                    }
                }
            }
            if partners != 1 {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}

fn permit(allowed: &mut [bool], range: Range<usize>) -> Result<(), EditError> {
    allowed
        .get_mut(range)
        .ok_or(EditError::Preservation)?
        .fill(true);
    Ok(())
}
