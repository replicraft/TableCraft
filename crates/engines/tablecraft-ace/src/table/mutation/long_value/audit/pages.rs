//! Exact header, old-body and initialized-free-space authority for touched pages.
use super::super::super::EditError;
use super::super::{maps::Family, stage::Staged};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{
            DELETED_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference_page,
            reference_slot, slot_offset,
        },
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const POINTER_BYTES: usize = size_of::<u32>();
const LVAL_OWNER: &[u8; size_of::<u32>()] = b"LVAL";

fn allow(
    ranges: &mut Vec<Range<usize>>,
    range: Range<usize>,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<Range<usize>>() as u64)
        .map_err(InspectError::from)?;
    ranges
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    ranges.push(range);

    Ok(())
}

pub(super) fn prove(
    before: &NativeImage,
    after: &NativeImage,
    schema: &TableSchema,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let plan = &staged.plan;
    for page in &plan.pages {
        check(budget, cancelled)?;
        let base = page.number as usize * PAGE_BYTES;
        let raw_before = if base < before.as_bytes().len() {
            before
                .as_bytes()
                .get(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?
        } else {
            &[]
        };
        let raw_after = after
            .as_bytes()
            .get(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if page.before != raw_before || page.after != raw_after {
            return Err(EditError::Preservation);
        }
        let owner = match page.family {
            Family::Ordinary => schema.definition_page().to_le_bytes(),
            Family::LongValue => *LVAL_OWNER,
        };
        if raw_after.get(..DATA_SIGNATURE.len()) != Some(DATA_SIGNATURE.as_slice())
            || raw_after.get(OWNER_OFFSET..OWNER_OFFSET + owner.len()) != Some(owner.as_slice())
        {
            return Err(EditError::Preservation);
        }
        if !raw_before.is_empty()
            && (raw_before.get(..DATA_SIGNATURE.len()) != Some(DATA_SIGNATURE.as_slice())
                || raw_before.get(OWNER_OFFSET..OWNER_OFFSET + owner.len())
                    != Some(owner.as_slice()))
        {
            return Err(EditError::Preservation);
        }
        let old_count = if raw_before.is_empty() {
            0
        } else {
            row_count(raw_before)?
        };
        let old_end = if old_count == 0 {
            PAGE_BYTES
        } else {
            row_range(raw_before, old_count - 1)?.1
        };
        let mut count = old_count;
        let mut end = old_end;
        let mut allowed = Vec::new();
        allow(
            &mut allowed,
            FREE_OFFSET..FREE_OFFSET + size_of::<u16>(),
            budget,
        )?;
        allow(
            &mut allowed,
            ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>(),
            budget,
        )?;
        budget
            .charge(Resource::Steps, plan.appended.len() as u64)
            .map_err(InspectError::from)?;
        for event in plan
            .appended
            .iter()
            .filter(|event| reference_page(event.reference) == page.number)
        {
            check(budget, cancelled)?;
            let start = event
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let stop = event
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let word = u16::try_from(start).map_err(|_| EditError::Preservation)?
                | if page.family == Family::Ordinary {
                    DELETED_MASK
                } else {
                    0
                };
            if event.family != page.family
                || usize::from(reference_slot(event.reference)) != count
                || stop != end
                || start < slot_offset(count + 1)
                || u16_at(raw_after, slot_offset(count))? != word
            {
                return Err(EditError::Preservation);
            }
            if page.family == Family::Ordinary {
                budget
                    .charge(Resource::Steps, staged.changes.len() as u64)
                    .map_err(InspectError::from)?;
                if staged
                    .changes
                    .iter()
                    .filter(|change| {
                        change.target == event.range
                            && change.target.len() > change.root.final_record.data.len()
                    })
                    .count()
                    != 1
                {
                    return Err(EditError::Preservation);
                }
            }
            allow(&mut allowed, start..stop, budget)?;
            allow(
                &mut allowed,
                slot_offset(count)..slot_offset(count + 1),
                budget,
            )?;
            count += 1;
            end = start;
        }
        if row_count(raw_after)? != count
            || usize::from(u16_at(raw_after, FREE_OFFSET)?)
                != end
                    .checked_sub(slot_offset(count))
                    .ok_or(EditError::Preservation)?
        {
            return Err(EditError::Preservation);
        }
        for change in &staged.changes {
            check(budget, cancelled)?;
            let root = &change.root;
            let old_target =
                root.final_record.offset..root.final_record.offset + root.final_record.data.len();
            if change.target.len() == old_target.len() {
                if change.target != old_target {
                    return Err(EditError::Preservation);
                }
                if change.target.start / PAGE_BYTES == page.number as usize {
                    if page.family != Family::Ordinary {
                        return Err(EditError::Preservation);
                    }
                    allow(
                        &mut allowed,
                        change.target.start - base..change.target.end - base,
                        budget,
                    )?;
                }
            } else {
                if change.target.len() < old_target.len() {
                    return Err(EditError::Preservation);
                }
                if reference_page(root.reference) == page.number {
                    if page.family != Family::Ordinary || root.range.len() < POINTER_BYTES {
                        return Err(EditError::Preservation);
                    }
                    let start = root.range.start - base;
                    let stop = root.range.end - base;
                    let slot = slot_offset(usize::from(reference_slot(root.reference)));
                    let destination = plan
                        .appended
                        .iter()
                        .find(|event| {
                            event.family == Family::Ordinary && event.range == change.target
                        })
                        .ok_or(EditError::Preservation)?;
                    if u16_at(raw_after, slot)? != root.word | RELOCATED_MASK
                        || u32_at(raw_after, start)? != destination.reference
                    {
                        return Err(EditError::Preservation);
                    }
                    budget
                        .charge(Resource::Steps, (stop - start) as u64)
                        .map_err(InspectError::from)?;
                    let tail = raw_after
                        .get(start + POINTER_BYTES..stop)
                        .ok_or(EditError::Preservation)?;
                    if root.word & RELOCATED_MASK == 0 {
                        if tail.iter().any(|&byte| byte != 0) {
                            return Err(EditError::Preservation);
                        }
                    } else if raw_before.get(start + POINTER_BYTES..stop) != Some(tail) {
                        return Err(EditError::Preservation);
                    }
                    allow(&mut allowed, start..stop, budget)?;
                    allow(&mut allowed, slot..slot + SLOT_BYTES, budget)?;
                }
            }
        }
        if raw_before.is_empty() {
            allow(&mut allowed, 0..DATA_SIGNATURE.len(), budget)?;
            allow(
                &mut allowed,
                OWNER_OFFSET..OWNER_OFFSET + owner.len(),
                budget,
            )?;
        }
        budget
            .charge(Resource::Steps, (PAGE_BYTES * (allowed.len() + 1)) as u64)
            .map_err(InspectError::from)?;
        for (offset, &byte) in raw_after.iter().enumerate() {
            if offset.is_multiple_of(256) {
                check(budget, cancelled)?;
            }
            if !allowed.iter().any(|range| range.contains(&offset))
                && byte != raw_before.get(offset).copied().unwrap_or(0)
            {
                return Err(EditError::Preservation);
            }
        }
    }

    let mut source = after.reader();
    let mut reader = Reader {
        source: &mut source,
        header: after.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for change in &staged.changes {
        reader.tick()?;
        let root = &change.root;
        let number = reference_page(root.reference);
        let page = reader.page(number)?;
        let actual = reader.row_location(
            number,
            reference_slot(root.reference),
            schema.definition_page(),
            &page,
        )?;
        if actual.offset != change.target.start || actual.data.len() != change.target.len() {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
