//! Explicit ordinary/LVAL page owners share only bounded physical append framing.
use super::super::{
    EditError, WriteBlocker,
    page_update::{roots::Root, space::Space},
};
use super::maps::{Family, Maps};
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
    limits::MAX_IMAGE_BYTES,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const LVAL_OWNER: &[u8; size_of::<u32>()] = b"LVAL";
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const POINTER_BYTES: usize = size_of::<u32>();
const EMPTY_FREE: usize = PAGE_BYTES - ROW_COUNT_OFFSET - size_of::<u16>();

pub(super) struct Page {
    pub number: u32,
    pub family: Family,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub(super) struct Attempt {
    pub family: Family,
    pub page: u32,
    pub bytes: usize,
    pub fitted: bool,
}

pub(super) struct Appended {
    pub family: Family,
    pub reference: u32,
    pub range: Range<usize>,
}

pub(super) struct Plan {
    pub maps: Maps,
    pub pages: Vec<Page>,
    pub attempts: Vec<Attempt>,
    pub appended: Vec<Appended>,
    pub original_length: usize,
    pub length: usize,
    owner: u32,
}

impl Plan {
    pub fn new(image: &NativeImage, owner: u32, maps: Maps) -> Self {
        Self {
            maps,
            pages: Vec::new(),
            attempts: Vec::new(),
            appended: Vec::new(),
            original_length: image.as_bytes().len(),
            length: image.as_bytes().len(),
            owner,
        }
    }

    pub fn geometry(bytes: &[u8]) -> Result<(usize, usize), EditError> {
        let count = row_count(bytes)?;
        let end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(bytes, count - 1)?.1
        };
        let free = end
            .checked_sub(slot_offset(count))
            .ok_or(InspectError::Malformed)?;
        if usize::from(u16_at(bytes, FREE_OFFSET)?) != free {
            return Err(InspectError::Malformed.into());
        }

        Ok((count, end))
    }

    fn owner_matches(&self, bytes: &[u8], family: Family) -> Result<bool, EditError> {
        if bytes.get(..DATA_SIGNATURE.len()) != Some(DATA_SIGNATURE.as_slice()) {
            return Ok(false);
        }
        match family {
            Family::Ordinary => Ok(u32_at(bytes, OWNER_OFFSET)? == self.owner),
            Family::LongValue => Ok(bytes.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_OWNER.len())
                == Some(LVAL_OWNER.as_slice())),
        }
    }

    fn push(
        &mut self,
        number: u32,
        family: Family,
        before: &[u8],
        after: &[u8],
        budget: &mut ResourceBudget,
    ) -> Result<usize, EditError> {
        budget
            .charge_allocation(1, size_of::<Page>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation((before.len() + after.len()) as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, (before.len() + after.len()) as u64)
            .map_err(InspectError::from)?;
        self.pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        let mut old = Vec::new();
        old.try_reserve_exact(before.len())
            .map_err(|_| InspectError::Allocation)?;
        old.extend_from_slice(before);
        let mut new = Vec::new();
        new.try_reserve_exact(after.len())
            .map_err(|_| InspectError::Allocation)?;
        new.extend_from_slice(after);
        self.pages.push(Page {
            number,
            family,
            before: old,
            after: new,
        });

        Ok(self.pages.len() - 1)
    }

    fn page(
        &mut self,
        image: &NativeImage,
        family: Family,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(position) = self.pages.iter().position(|page| page.number == number) {
            if self.pages[position].family != family {
                return Err(EditError::Preservation);
            }
            return Ok(position);
        }

        let start = number as usize * PAGE_BYTES;
        let bytes = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        if !self.owner_matches(bytes, family)? {
            return Err(InspectError::Malformed.into());
        }
        Self::geometry(bytes)?;
        self.push(number, family, bytes, bytes, budget)
    }

    fn destination(
        &mut self,
        image: &NativeImage,
        family: Family,
        bytes: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        if bytes
            .checked_add(SLOT_BYTES)
            .is_none_or(|needed| needed > EMPTY_FREE)
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        while let Some(number) = self.maps.candidate(image, family, budget, cancelled)? {
            let position = self.page(image, family, number, budget, cancelled)?;
            let (count, end) = Self::geometry(&self.pages[position].after)?;
            let fitted = count < MAX_SLOTS && bytes + SLOT_BYTES <= end - slot_offset(count);
            budget
                .charge_allocation(1, size_of::<Attempt>() as u64)
                .map_err(InspectError::from)?;
            self.attempts
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            self.attempts.push(Attempt {
                family,
                page: number,
                bytes,
                fitted,
            });
            if fitted {
                return Ok(position);
            }
            self.maps
                .failed_fit(image, family, number, budget, cancelled)?;
        }

        let length = self
            .length
            .checked_add(PAGE_BYTES)
            .filter(|&length| length as u64 <= MAX_IMAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let number =
            u32::try_from(self.length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
        self.maps
            .allocate(image, family, number, budget, cancelled)?;
        budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut initial = Vec::new();
        initial
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        initial.resize(PAGE_BYTES, 0);
        initial[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        initial[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
        let owner = match family {
            Family::Ordinary => self.owner.to_le_bytes(),
            Family::LongValue => *LVAL_OWNER,
        };
        initial[OWNER_OFFSET..OWNER_OFFSET + owner.len()].copy_from_slice(&owner);
        let position = self.push(number, family, &[], &initial, budget)?;
        self.length = length;

        Ok(position)
    }

    pub fn append(
        &mut self,
        image: &NativeImage,
        family: Family,
        body: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        check(budget, cancelled)?;
        let position = self.destination(image, family, body.len(), budget, cancelled)?;
        let page = &mut self.pages[position];
        let (count, end) = Self::geometry(&page.after)?;
        let slot = u8::try_from(count).map_err(|_| InspectError::Malformed)?;
        let start = end.checked_sub(body.len()).ok_or(EditError::Preservation)?;
        let word = u16::try_from(start).map_err(|_| InspectError::Malformed)?
            | match family {
                Family::Ordinary => DELETED_MASK,
                Family::LongValue => 0,
            };
        budget
            .charge(Resource::Steps, body.len() as u64)
            .map_err(InspectError::from)?;
        page.after[start..end].copy_from_slice(body);
        page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&word.to_le_bytes());
        page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].copy_from_slice(
            &(u16::try_from(count + 1).map_err(|_| InspectError::Malformed)?).to_le_bytes(),
        );
        let free = start - slot_offset(count + 1);
        page.after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
            &(u16::try_from(free).map_err(|_| InspectError::Malformed)?).to_le_bytes(),
        );
        budget
            .charge_allocation(1, size_of::<Appended>() as u64)
            .map_err(InspectError::from)?;
        self.appended
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        let base = page.number as usize * PAGE_BYTES;
        self.appended.push(Appended {
            family,
            reference: reference(page.number, slot),
            range: base + start..base + end,
        });

        Ok(self.appended.len() - 1)
    }

    pub fn ordinary(
        &mut self,
        image: &NativeImage,
        root: &Root<'_>,
        record: &[u8],
        space: &Space,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Range<usize>, EditError> {
        check(budget, cancelled)?;
        let root_page = reference_page(root.reference);
        let final_page = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| InspectError::Malformed)?;
        space.require_page(image, root_page, false, budget, cancelled)?;
        space.require_page(image, final_page, false, budget, cancelled)?;
        if record.len() < root.final_record.data.len() {
            return Err(EditError::Preservation);
        }
        if record.len() == root.final_record.data.len() {
            let position = self.page(image, Family::Ordinary, final_page, budget, cancelled)?;
            let page = &mut self.pages[position];
            let start = root.final_record.offset - final_page as usize * PAGE_BYTES;
            let end = start + record.len();
            if page.after.get(start..end) != Some(root.final_record.data.as_slice()) {
                return Err(EditError::Preservation);
            }
            budget
                .charge(Resource::Steps, record.len() as u64)
                .map_err(InspectError::from)?;
            page.after[start..end].copy_from_slice(record);
            return Ok(root.final_record.offset..root.final_record.offset + record.len());
        }
        if root.range.len() < POINTER_BYTES || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let appended = self.append(image, Family::Ordinary, record, budget, cancelled)?;
        let destination = self.appended[appended].reference;
        let target = self.appended[appended].range.clone();
        let position = self.page(image, Family::Ordinary, root_page, budget, cancelled)?;
        let page = &mut self.pages[position];
        let base = root_page as usize * PAGE_BYTES;
        let range = root.range.start - base..root.range.end - base;
        let word = slot_offset(usize::from(reference_slot(root.reference)));
        if u16_at(&page.after, word)? != root.word
            || page.after.get(range.clone()) != image.as_bytes().get(root.range.clone())
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, range.len() as u64)
            .map_err(InspectError::from)?;
        if root.word & RELOCATED_MASK == 0 {
            page.after[range.clone()].fill(0);
        }
        page.after[range.start..range.start + POINTER_BYTES]
            .copy_from_slice(&destination.to_le_bytes());
        page.after[word..word + SLOT_BYTES]
            .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());

        Ok(target)
    }

    pub fn bytes_mut(
        &mut self,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&mut [u8], EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        let number = range.start / PAGE_BYTES;
        let page = self
            .pages
            .iter_mut()
            .find(|page| page.number as usize == number)
            .ok_or(EditError::Preservation)?;
        let base = number * PAGE_BYTES;
        let start = range
            .start
            .checked_sub(base)
            .ok_or(EditError::Preservation)?;
        let end = range.end.checked_sub(base).ok_or(EditError::Preservation)?;
        page.after
            .get_mut(start..end)
            .ok_or(EditError::Preservation)
    }
}
