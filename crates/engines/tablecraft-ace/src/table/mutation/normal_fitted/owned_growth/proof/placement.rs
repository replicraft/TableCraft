//! Independent original-directory reservation and two-page reconstruction.
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES, PAGE_NUMBER_MASK},
        reader::row_count,
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, RELOCATED_MASK, ROW_COUNT_OFFSET, SLOT_BYTES,
            UNSUPPORTED_MASK, reference, reference_page, reference_slot, slot_offset,
        },
    },
    header::check,
    table::mutation::{
        normal_fitted::binding::Selected, record_placement::space::Space, record_roots::Root,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_BYTES_OFFSET: usize = 2;
const POINTER_BYTES: usize = size_of::<u32>();

struct PageState {
    number: u32,
    eligible: bool,
    slots: usize,
    boundary: usize,
    free: usize,
}

pub(in crate::table::mutation) struct Placement {
    pages: Vec<PageState>,
}

pub(in crate::table::mutation) struct Choice {
    page: u32,
    slot: Option<u8>,
    target: Range<usize>,
}

impl Choice {
    pub(in crate::table::mutation) fn page_number(&self) -> u32 {
        self.page
    }
}

impl Placement {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        space: &Space,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        budget
            .charge_allocation(owned.len() as u64, size_of::<PageState>() as u64)
            .map_err(InspectError::from)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(owned.len())
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = None;
        for &number in owned {
            check(budget, cancelled)?;
            if previous.is_some_and(|before| before >= number) {
                return Err(EditError::Preservation);
            }
            previous = Some(number);
            if number & !PAGE_NUMBER_MASK != 0 {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let eligible = space.append_eligible(image, number, budget, cancelled)?;
            let base = usize::try_from(number)
                .ok()
                .and_then(|number| number.checked_mul(PAGE_BYTES))
                .ok_or(EditError::Preservation)?;
            let raw = image
                .as_bytes()
                .get(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(raw, OWNER_OFFSET)? != space.definition()
            {
                return Err(InspectError::Malformed.into());
            }
            let slots = row_count(raw)?;
            let boundary = boundary(raw, budget, cancelled)?;
            let free = usize::from(u16_at(raw, FREE_BYTES_OFFSET)?);
            if boundary.checked_sub(slot_offset(slots)) != Some(free) {
                return Err(InspectError::Malformed.into());
            }
            pages.push(PageState {
                number,
                eligible,
                slots,
                boundary,
                free,
            });
        }
        Ok(Self { pages })
    }

    pub(in crate::table::mutation) fn include_eof(
        &mut self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if number & !PAGE_NUMBER_MASK != 0
            || self.pages.last().is_some_and(|page| page.number >= number)
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(1, size_of::<PageState>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge(
                Resource::Steps,
                self.pages
                    .len()
                    .checked_mul(size_of::<PageState>())
                    .ok_or(InspectError::StructureLimit)? as u64,
            )
            .map_err(InspectError::from)?;
        self.pages
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        self.pages.push(PageState {
            number,
            eligible: true,
            slots: 0,
            boundary: PAGE_BYTES,
            free: PAGE_BYTES - slot_offset(0),
        });
        Ok(())
    }

    pub(in crate::table::mutation) fn reserve(
        &mut self,
        root: &Root<'_, Selected>,
        row: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Choice, EditError> {
        self.try_reserve(root, row, budget, cancelled)?
            .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))
    }

    pub(in crate::table::mutation) fn try_reserve(
        &mut self,
        root: &Root<'_, Selected>,
        row: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Choice>, EditError> {
        check(budget, cancelled)?;
        if row.len() < root.final_record.data.len() {
            return Err(EditError::Preservation);
        }
        if row.len() == root.final_record.data.len() {
            return Ok(Some(Choice {
                page: u32::try_from(root.final_record.offset / PAGE_BYTES)
                    .map_err(|_| EditError::Preservation)?,
                slot: None,
                target: root.final_record.offset..root.final_record.offset + row.len(),
            }));
        }
        let root_page = reference_page(root.reference);
        let mut position = None;
        let mut found_root = false;
        for (ordinal, page) in self.pages.iter().enumerate() {
            check(budget, cancelled)?;
            if page.number == root_page {
                found_root = true;
                if available(page, row.len())? {
                    position = Some(ordinal);
                }
            }
        }
        if !found_root {
            return Err(EditError::Preservation);
        }
        if position.is_none() {
            for (ordinal, page) in self.pages.iter().enumerate() {
                check(budget, cancelled)?;
                if page.number != root_page && available(page, row.len())? {
                    position = Some(ordinal);
                    break;
                }
            }
        }
        let Some(position) = position else {
            return Ok(None);
        };
        let page = self
            .pages
            .get_mut(position)
            .ok_or(EditError::Preservation)?;
        let slot = u8::try_from(page.slots).map_err(|_| EditError::Preservation)?;
        let end = page.boundary;
        page.boundary = end.checked_sub(row.len()).ok_or(EditError::Preservation)?;
        page.free = page
            .free
            .checked_sub(row.len() + SLOT_BYTES)
            .ok_or(EditError::Preservation)?;
        page.slots += 1;
        let base = page.number as usize * PAGE_BYTES;
        Ok(Some(Choice {
            page: page.number,
            slot: Some(slot),
            target: base + page.boundary..base + end,
        }))
    }
}

fn available(page: &PageState, bytes: usize) -> Result<bool, EditError> {
    let required = bytes
        .checked_add(SLOT_BYTES)
        .ok_or(EditError::Preservation)?;
    Ok(page.eligible
        && page.slots < MAX_SLOTS
        && page.free >= required
        && page
            .boundary
            .checked_sub(bytes)
            .is_some_and(|start| start >= slot_offset(page.slots + 1)))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep the independent choice separate from original root and row bytes."
)]
pub(in crate::table::mutation) fn apply(
    image: &NativeImage,
    space: &Space,
    root: &Root<'_, Selected>,
    row: &[u8],
    choice: &Choice,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    apply_with(
        image, space, root, row, choice, None, expected, budget, cancelled,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep original proof state and allocation qualification distinct."
)]
pub(in crate::table::mutation) fn apply_eof(
    image: &NativeImage,
    space: &Space,
    root: &Root<'_, Selected>,
    row: &[u8],
    choice: &Choice,
    future: &crate::table::mutation::record_placement::future::Future,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    apply_with(
        image,
        space,
        root,
        row,
        choice,
        Some(future),
        expected,
        budget,
        cancelled,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Independent physical proof combines source, choice and optional fresh EOF qualification."
)]
fn apply_with(
    image: &NativeImage,
    space: &Space,
    root: &Root<'_, Selected>,
    row: &[u8],
    choice: &Choice,
    future: Option<&crate::table::mutation::record_placement::future::Future>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let root_page = reference_page(root.reference);
    let terminal_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
        .map_err(|_| EditError::Preservation)?;
    space.require_page(image, root_page, false, budget, cancelled)?;
    space.require_page(image, terminal_page, false, budget, cancelled)?;
    let Some(slot) = choice.slot else {
        if row.len() != root.final_record.data.len()
            || choice.target.start != root.final_record.offset
        {
            return Err(EditError::Preservation);
        }
        let target = expected
            .get_mut(choice.target.clone())
            .ok_or(EditError::Preservation)?;
        budget
            .charge(Resource::Steps, row.len() as u64)
            .map_err(InspectError::from)?;
        if target != root.final_record.data.as_slice() {
            return Err(EditError::Preservation);
        }
        target.copy_from_slice(row);
        return Ok(());
    };
    if row.len() <= root.final_record.data.len()
        || root.range.len() < POINTER_BYTES
        || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
    {
        return Err(EditError::Preservation);
    }
    if let Some(future) = future.filter(|future| future.number() == choice.page) {
        future.require(image, space.definition())?;
        if expected.len() != image.as_bytes().len() + PAGE_BYTES {
            return Err(EditError::Preservation);
        }
    } else {
        space.require_page(image, choice.page, true, budget, cancelled)?;
    }
    {
        let base = choice.page as usize * PAGE_BYTES;
        let page = expected
            .get_mut(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let count = row_count(page)?;
        let end = boundary(page, budget, cancelled)?;
        let start = choice
            .target
            .start
            .checked_sub(base)
            .ok_or(EditError::Preservation)?;
        let stop = choice
            .target
            .end
            .checked_sub(base)
            .ok_or(EditError::Preservation)?;
        let free = usize::from(u16_at(page, FREE_BYTES_OFFSET)?);
        if count != usize::from(slot)
            || count >= MAX_SLOTS
            || stop != end
            || stop.checked_sub(start) != Some(row.len())
            || start < slot_offset(count + 1)
            || end.checked_sub(slot_offset(count)) != Some(free)
            || free < row.len() + SLOT_BYTES
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, row.len() as u64)
            .map_err(InspectError::from)?;
        page.get_mut(start..stop)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(row);
        let word = u16::try_from(start).map_err(|_| EditError::Preservation)? | DELETED_MASK;
        page[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&word.to_le_bytes());
        page[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].copy_from_slice(
            &u16::try_from(count + 1)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
        page[FREE_BYTES_OFFSET..FREE_BYTES_OFFSET + size_of::<u16>()].copy_from_slice(
            &u16::try_from(free - row.len() - SLOT_BYTES)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
    }
    {
        let base = root_page as usize * PAGE_BYTES;
        let page = expected
            .get_mut(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
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
        if u16_at(page, word_at)? != root.word
            || page.get(start..end) != image.as_bytes().get(root.range.clone())
        {
            return Err(EditError::Preservation);
        }
        if root.word & RELOCATED_MASK == 0 {
            page[start..end].fill(0);
        }
        page[start..start + POINTER_BYTES]
            .copy_from_slice(&reference(choice.page, slot).to_le_bytes());
        page[word_at..word_at + SLOT_BYTES]
            .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());
    }
    check(budget, cancelled)?;
    Ok(())
}

fn boundary(
    page: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let count = row_count(page)?;
    let mut end = page.len();
    for slot in 0..count {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<u16>() as u64)
            .map_err(InspectError::from)?;
        let word = u16_at(page, slot_offset(slot))?;
        if word & UNSUPPORTED_MASK != 0 {
            return Err(InspectError::Unsupported.into());
        }
        let start = usize::from(word & OFFSET_MASK);
        if start < slot_offset(count) || start > end || (start == end && word & DELETED_MASK == 0) {
            return Err(InspectError::Malformed.into());
        }
        end = start;
    }
    Ok(end)
}
