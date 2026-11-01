//! Append hidden targets without moving or reusing any original physical slot.
use super::super::{EditError, WriteBlocker, patch::BytePatch, rows::RowAssignment};
use super::{slots, space::Space};
use crate::table::mutation::record_roots::Root;
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::row_count,
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES,
            reference, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) const FREE_OFFSET: usize = DATA_SIGNATURE.len();
pub(in crate::table::mutation) const POINTER_BYTES: usize = size_of::<u32>();

#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::table::mutation) enum EventKind {
    SamePage,
    #[cfg(feature = "journal")]
    AppendTarget,
    #[cfg(feature = "journal")]
    ForwardRoot,
}

pub(in crate::table::mutation) struct Event {
    pub kind: EventKind,
    pub root: u32,
    pub root_word: u16,
    pub root_range: Range<usize>,
    pub target: Range<usize>,
    pub appended: Option<u8>,
}

pub(in crate::table::mutation) struct Page {
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

pub(in crate::table::mutation) struct Plan {
    pub pages: Vec<Page>,
    pub(super) cross_page: bool,
}

impl Plan {
    pub(in crate::table::mutation) fn new() -> Self {
        Self {
            pages: Vec::new(),
            cross_page: false,
        }
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
        let start = slots::frontier(raw, budget, cancelled)?;
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

    pub(in crate::table::mutation) fn apply<U: RowAssignment>(
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
            let end = slots::frontier(&page.after, budget, cancelled)?;
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
            kind: EventKind::SamePage,
            root: root.reference,
            root_word: root.word,
            root_range: root.range.clone(),
            target: target.clone(),
            appended,
        });
        check(budget, cancelled)?;
        Ok(target)
    }

    /// Consume a private original-image reservation; legacy apply stays same-root.
    #[cfg(feature = "journal")]
    #[expect(
        clippy::too_many_arguments,
        reason = "Keep original root, bytes, protection and opaque reservation distinct."
    )]
    pub(in crate::table::mutation) fn apply_reserved<U: RowAssignment>(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, U>,
        record: &[u8],
        space: &Space,
        reservation: &super::owned::Reservation,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError> {
        reservation.require(image, space.definition(), root, record.len())?;
        let Some(destination) = reservation.destination() else {
            return self.apply(image, root, record, space, budget, cancelled);
        };
        let root_page = reference_page(root.reference);
        if destination.page == root_page {
            let target = self.apply(image, root, record, space, budget, cancelled)?;
            if target != destination.range {
                return Err(EditError::Preservation);
            }
            return Ok(target);
        }
        let final_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        space.require_page(image, root_page, false, budget, cancelled)?;
        space.require_page(image, final_page, false, budget, cancelled)?;
        space.require_page(image, destination.page, true, budget, cancelled)?;
        if record.len() <= root.final_record.data.len()
            || root.range.len() < POINTER_BYTES
            || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
        {
            return Err(EditError::Preservation);
        }
        {
            let page = self.page(
                image,
                space.definition(),
                destination.page,
                budget,
                cancelled,
            )?;
            let count = row_count(&page.after)?;
            let end = slots::frontier(&page.after, budget, cancelled)?;
            let free = usize::from(u16_at(&page.after, FREE_OFFSET)?);
            let base = destination.page as usize * PAGE_BYTES;
            let start = destination
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let stop = destination
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            if count != usize::from(destination.slot)
                || count >= MAX_SLOTS
                || stop != end
                || stop.checked_sub(start) != Some(record.len())
                || start < slot_offset(count + 1)
                || free != end - slot_offset(count)
                || free < record.len() + SLOT_BYTES
            {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, record.len() as u64)
                .map_err(InspectError::from)?;
            page.after
                .get_mut(start..stop)
                .ok_or(EditError::Preservation)?
                .copy_from_slice(record);
            page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(
                &(u16::try_from(start).map_err(|_| EditError::Preservation)? | DELETED_MASK)
                    .to_le_bytes(),
            );
            page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].copy_from_slice(
                &u16::try_from(count + 1)
                    .map_err(|_| EditError::Preservation)?
                    .to_le_bytes(),
            );
            page.after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
                &u16::try_from(free - record.len() - SLOT_BYTES)
                    .map_err(|_| EditError::Preservation)?
                    .to_le_bytes(),
            );
            budget
                .charge_allocation(1, size_of::<Event>() as u64)
                .map_err(InspectError::from)?;
            page.events
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            page.events.push(Event {
                kind: EventKind::AppendTarget,
                root: root.reference,
                root_word: root.word,
                root_range: root.range.clone(),
                target: destination.range.clone(),
                appended: Some(destination.slot),
            });
        }
        {
            let page = self.page(image, space.definition(), root_page, budget, cancelled)?;
            let base = root_page as usize * PAGE_BYTES;
            let start = root
                .range
                .start
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let end = root
                .range
                .end
                .checked_sub(base)
                .ok_or(EditError::Preservation)?;
            let word_at = slot_offset(usize::from(reference_slot(root.reference)));
            budget
                .charge(Resource::Steps, root.range.len() as u64)
                .map_err(InspectError::from)?;
            if u16_at(&page.after, word_at)? != root.word
                || page.after.get(start..end) != image.as_bytes().get(root.range.clone())
            {
                return Err(EditError::Preservation);
            }
            if root.word & RELOCATED_MASK == 0 {
                page.after[start..end].fill(0);
            }
            page.after[start..start + POINTER_BYTES]
                .copy_from_slice(&reference(destination.page, destination.slot).to_le_bytes());
            page.after[word_at..word_at + SLOT_BYTES]
                .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());
            budget
                .charge_allocation(1, size_of::<Event>() as u64)
                .map_err(InspectError::from)?;
            page.events
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            page.events.push(Event {
                kind: EventKind::ForwardRoot,
                root: root.reference,
                root_word: root.word,
                root_range: root.range.clone(),
                target: destination.range.clone(),
                appended: Some(destination.slot),
            });
        }
        self.cross_page = true;
        check(budget, cancelled)?;
        Ok(destination.range)
    }
}
