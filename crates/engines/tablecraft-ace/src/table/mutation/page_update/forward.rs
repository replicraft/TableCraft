//! Append hidden targets without moving or reusing any original physical slot.
use super::super::{EditError, WriteBlocker, patch::BytePatch, rows::RowAssignment};
use super::{roots::Root, space::Space};
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES,
            reference, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const FREE_OFFSET: usize = DATA_SIGNATURE.len();
pub(super) const POINTER_BYTES: usize = size_of::<u32>();

pub(super) struct Event {
    pub root: u32,
    pub root_word: u16,
    pub root_range: Range<usize>,
    pub target: Range<usize>,
    pub appended: Option<u8>,
}

pub(super) struct Page {
    pub number: u32,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub events: Vec<Event>,
}

impl BytePatch for Page {
    fn offset(&self) -> usize {
        self.number as usize * PAGE_BYTES
    }

    fn before(&self) -> &[u8] {
        &self.before
    }

    fn after(&self) -> &[u8] {
        &self.after
    }

    fn requires_structural_check(&self) -> bool {
        true
    }
}

pub(super) struct Plan {
    pub pages: Vec<Page>,
}

impl Plan {
    pub(super) fn new() -> Self {
        Self { pages: Vec::new() }
    }

    fn page(
        &mut self,
        image: &NativeImage,
        owner: u32,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&mut Page, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(position) = self.pages.iter().position(|page| page.number == number) {
            return Ok(&mut self.pages[position]);
        }
        let offset = number as usize * PAGE_BYTES;
        let raw = image
            .bytes
            .get(offset..offset + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE || u32_at(raw, OWNER_OFFSET)? != owner {
            return Err(InspectError::Malformed.into());
        }
        let count = row_count(raw)?;
        let start = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(raw, count - 1)?.1
        };
        if usize::from(u16_at(raw, FREE_OFFSET)?) != start - slot_offset(count) {
            return Err(InspectError::Malformed.into());
        }
        budget
            .charge_allocation(1, size_of::<Page>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation((2 * PAGE_BYTES) as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
            .map_err(InspectError::from)?;
        let mut before = Vec::new();
        before
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        before.extend_from_slice(raw);
        let mut after = Vec::new();
        after
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        after.extend_from_slice(raw);
        self.pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.pages.push(Page {
            number,
            before,
            after,
            events: Vec::new(),
        });
        self.pages.last_mut().ok_or(EditError::Preservation)
    }

    pub(super) fn apply<U: RowAssignment>(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, U>,
        record: &[u8],
        space: &Space,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError> {
        let root_page = reference_page(root.reference);
        let final_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| InspectError::Malformed)?;
        space.require_page(image, root_page, false, budget, cancelled)?;
        space.require_page(image, final_page, false, budget, cancelled)?;
        if record.len() < root.final_record.data.len() {
            return Err(EditError::Preservation);
        }
        let appending = record.len() > root.final_record.data.len();
        let number = if appending { root_page } else { final_page };
        space.require_page(image, number, appending, budget, cancelled)?;
        let page = self.page(image, space.definition(), number, budget, cancelled)?;
        budget
            .charge_allocation(1, size_of::<Event>() as u64)
            .map_err(InspectError::from)?;
        page.events
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        let (target, appended) = if !appending {
            let start = root.final_record.offset - number as usize * PAGE_BYTES;
            let end = start
                .checked_add(record.len())
                .ok_or(InspectError::Malformed)?;
            let before = page
                .after
                .get_mut(start..end)
                .ok_or(InspectError::Malformed)?;
            if before != root.final_record.data.as_slice() {
                return Err(EditError::Preservation);
            }
            before.copy_from_slice(record);
            (
                root.final_record.offset..root.final_record.offset + record.len(),
                None,
            )
        } else {
            if root.range.len() < POINTER_BYTES || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }

            let count = row_count(&page.after)?;
            if count >= MAX_SLOTS {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }

            let slot =
                u8::try_from(count).map_err(|_| EditError::Unsupported(WriteBlocker::RowLayout))?;
            let end = if count == 0 {
                PAGE_BYTES
            } else {
                row_range(&page.after, count - 1)?.1
            };
            let start = end
                .checked_sub(record.len())
                .filter(|start| *start >= slot_offset(count + 1))
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            let free = usize::from(u16_at(&page.after, FREE_OFFSET)?);
            if free != end - slot_offset(count) || free < record.len() + SLOT_BYTES {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }

            let base = number as usize * PAGE_BYTES;
            let root_start = root
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let root_end = root
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let root_word_at = slot_offset(usize::from(reference_slot(root.reference)));
            if u16_at(&page.after, root_word_at)? != root.word
                || page
                    .after
                    .get(root_start..root_end)
                    .ok_or(EditError::Preservation)?
                    != image
                        .bytes
                        .get(root.range.clone())
                        .ok_or(EditError::Preservation)?
            {
                return Err(EditError::Preservation);
            }

            page.after[start..end].copy_from_slice(record);
            page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(
                &(u16::try_from(start).map_err(|_| InspectError::Malformed)? | DELETED_MASK)
                    .to_le_bytes(),
            );

            page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].copy_from_slice(
                &u16::try_from(count + 1)
                    .map_err(|_| InspectError::Malformed)?
                    .to_le_bytes(),
            );
            page.after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
                &u16::try_from(free - record.len() - SLOT_BYTES)
                    .map_err(|_| InspectError::Malformed)?
                    .to_le_bytes(),
            );

            if root.word & RELOCATED_MASK == 0 {
                page.after[root_start..root_end].fill(0);
            }
            page.after[root_start..root_start + POINTER_BYTES]
                .copy_from_slice(&reference(number, slot).to_le_bytes());
            page.after[root_word_at..root_word_at + SLOT_BYTES]
                .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());

            (base + start..base + end, Some(slot))
        };

        page.events.push(Event {
            root: root.reference,
            root_word: root.word,
            root_range: root.range.clone(),
            target: target.clone(),
            appended,
        });
        check(budget, cancelled)?;
        Ok(target)
    }
}
