//! Real alternate-page placement and bounded EOF data-page initialization.
use super::super::{
    EditError, SlotScalarUpdate, WriteBlocker,
    append::State,
    page_update::{roots::Root, space::Space, stage::Placement},
    variable_split::metadata::Metadata,
};
use super::maps::Maps;
use crate::{
    InspectError, NativeImage, TableSchema,
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

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const POINTER_BYTES: usize = size_of::<u32>();
const EMPTY_FREE: usize = PAGE_BYTES - ROW_COUNT_OFFSET - size_of::<u16>();

pub(in crate::table::mutation) struct Page {
    pub number: u32,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub(in crate::table::mutation) struct Event {
    pub(in crate::table::mutation) root: u32,
    pub(in crate::table::mutation) word: u16,
    pub(in crate::table::mutation) root_range: Range<usize>,
    pub(in crate::table::mutation) target: Range<usize>,
    pub(in crate::table::mutation) appended: Option<u8>,
}

pub(in crate::table::mutation) struct Plan {
    pub pages: Vec<Page>,
    pub maps: Maps,
    pub append: State,
    pub(super) events: Vec<Event>,
    metadata: Option<Metadata>,
}

pub(in crate::table::mutation) struct Data {
    pub pages: Vec<Page>,
    pub maps: Maps,
    pub events: Vec<Event>,
    pub length: usize,
}

#[derive(Clone, Copy)]
pub(in crate::table::mutation) struct DataView<'a> {
    pub pages: &'a [Page],
    pub maps: &'a Maps,
    pub events: &'a [Event],
    pub append: &'a State,
    pub length: usize,
}

impl Data {
    pub(in crate::table::mutation) fn view<'a>(&'a self, append: &'a State) -> DataView<'a> {
        DataView {
            pages: &self.pages,
            maps: &self.maps,
            events: &self.events,
            append,
            length: self.length,
        }
    }

    pub(in crate::table::mutation) fn audit_membership(
        &self,
        after: &[u8],
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.maps.audit_membership(after, schema, budget, cancelled)
    }
}

impl<'a> DataView<'a> {
    pub(in crate::table::mutation) fn target(
        self,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&'a [u8], EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        let number = range.start / PAGE_BYTES;
        let page = self
            .pages
            .iter()
            .find(|page| page.number as usize == number)
            .ok_or(EditError::Preservation)?;
        let base = number * PAGE_BYTES;
        page.after
            .get(range.start - base..range.end - base)
            .ok_or(EditError::Preservation)
    }
}

impl Plan {
    pub(in crate::table::mutation) fn view(&self) -> DataView<'_> {
        DataView {
            pages: &self.pages,
            maps: &self.maps,
            events: &self.events,
            append: &self.append,
            length: self.append.length,
        }
    }

    pub(in crate::table::mutation) fn into_parts(self) -> (Data, State) {
        let data = Data {
            pages: self.pages,
            maps: self.maps,
            events: self.events,
            length: self.append.length,
        };
        (data, self.append)
    }

    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Ok(Self {
            pages: Vec::new(),
            maps: Maps::capture(image, schema, budget, cancelled)?,
            append: State::new(image, schema.definition),
            events: Vec::new(),
            metadata: None,
        })
    }

    pub(in crate::table::mutation) fn capture_protected(
        image: &NativeImage,
        schema: &TableSchema,
        metadata: Metadata,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut plan = Self::capture(image, schema, budget, cancelled)?;
        plan.maps
            .enable_hint_growth(image, schema, &metadata, budget, cancelled)?;
        plan.metadata = Some(metadata);
        Ok(plan)
    }

    fn page(
        &mut self,
        image: &NativeImage,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.pages.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(position) = self.pages.iter().position(|page| page.number == number) {
            return Ok(position);
        }
        let start = number as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(raw, OWNER_OFFSET)? != self.append.owner
        {
            return Err(InspectError::Malformed.into());
        }
        Self::geometry(raw)?;
        self.push_page(number, raw, raw, budget)?;
        Ok(self.pages.len() - 1)
    }

    fn push_page(
        &mut self,
        number: u32,
        before: &[u8],
        after: &[u8],
        budget: &mut ResourceBudget,
    ) -> Result<(), EditError> {
        budget
            .charge_allocation(1, size_of::<Page>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation((before.len() + after.len()) as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, (before.len() + after.len()) as u64)
            .map_err(InspectError::from)?;
        let mut original = Vec::new();
        original
            .try_reserve_exact(before.len())
            .map_err(|_| InspectError::Allocation)?;
        original.extend_from_slice(before);
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(after.len())
            .map_err(|_| InspectError::Allocation)?;
        replacement.extend_from_slice(after);
        self.pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.pages.push(Page {
            number,
            before: original,
            after: replacement,
        });
        Ok(())
    }

    fn geometry(raw: &[u8]) -> Result<(usize, usize), EditError> {
        let count = row_count(raw)?;
        let end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(raw, count - 1)?.1
        };
        if usize::from(u16_at(raw, FREE_OFFSET)?) != end - slot_offset(count) {
            return Err(InspectError::Malformed.into());
        }
        Ok((count, end))
    }

    fn destination(
        &mut self,
        image: &NativeImage,
        length: usize,
        space: &Space,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        if length + SLOT_BYTES > EMPTY_FREE {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if let Some(number) = self
            .maps
            .candidate(image, &self.append, budget, cancelled)?
        {
            space.require_metadata_page(number, budget, cancelled)?;
            let position = self.page(image, number, budget, cancelled)?;
            let (count, end) = Self::geometry(&self.pages[position].after)?;
            if count < MAX_SLOTS && length + SLOT_BYTES <= end - slot_offset(count) {
                return Ok(position);
            }
            self.maps
                .failed_fit(image, number, &mut self.append, budget, cancelled)?;
        }
        self.append.preview(1)?;
        let number = self.append.next()?;
        self.maps
            .allocate(image, number, &mut self.append, budget, cancelled)?;
        budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        let mut initial = Vec::new();
        initial
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        initial.resize(PAGE_BYTES, 0);
        initial[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        initial[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
        initial[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
            .copy_from_slice(&self.append.owner.to_le_bytes());
        self.push_page(number, &[], &initial, budget)?;
        Ok(self.pages.len() - 1)
    }
}

impl Placement for Plan {
    fn capture_space(
        &mut self,
        image: &NativeImage,
        schema: &TableSchema,
        owned: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Space, EditError> {
        let space = Space::capture(image, schema, owned, budget, cancelled)?;
        Ok(match self.metadata.take() {
            Some(metadata) => space.with_metadata(metadata),
            None => space,
        })
    }

    fn apply(
        &mut self,
        image: &NativeImage,
        root: &Root<'_, SlotScalarUpdate>,
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
        let growing = record.len() > root.final_record.data.len();
        let target;
        let appended;
        if growing {
            if root.range.len() < POINTER_BYTES || root.word & !(OFFSET_MASK | RELOCATED_MASK) != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let position = self.destination(image, record.len(), space, budget, cancelled)?;
            let page = &mut self.pages[position];
            let (count, end) = Self::geometry(&page.after)?;
            let slot = u8::try_from(count).map_err(|_| InspectError::Malformed)?;
            let start = end
                .checked_sub(record.len())
                .ok_or(EditError::Preservation)?;
            let base = page.number as usize * PAGE_BYTES;
            page.after[start..end].copy_from_slice(record);
            page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(
                &(u16::try_from(start).map_err(|_| InspectError::Malformed)? | DELETED_MASK)
                    .to_le_bytes(),
            );
            page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + size_of::<u16>()].copy_from_slice(
                &(u16::try_from(count + 1).map_err(|_| InspectError::Malformed)?).to_le_bytes(),
            );
            let free = start - slot_offset(count + 1);
            page.after[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
                &(u16::try_from(free).map_err(|_| InspectError::Malformed)?).to_le_bytes(),
            );
            target = base + start..base + end;
            appended = Some(slot);
            let destination = reference(page.number, slot);

            let position = self.page(image, root_page, budget, cancelled)?;
            let page = &mut self.pages[position];
            let base = root_page as usize * PAGE_BYTES;
            let range = root.range.start - base..root.range.end - base;
            let word_offset = slot_offset(usize::from(reference_slot(root.reference)));
            if u16_at(&page.after, word_offset)? != root.word
                || page.after.get(range.clone()) != image.as_bytes().get(root.range.clone())
            {
                return Err(EditError::Preservation);
            }
            if root.word & RELOCATED_MASK == 0 {
                page.after[range.clone()].fill(0);
            }
            page.after[range.start..range.start + POINTER_BYTES]
                .copy_from_slice(&destination.to_le_bytes());
            page.after[word_offset..word_offset + SLOT_BYTES]
                .copy_from_slice(&(root.word | RELOCATED_MASK).to_le_bytes());
        } else {
            let position = self.page(image, final_page, budget, cancelled)?;
            let page = &mut self.pages[position];
            let base = final_page as usize * PAGE_BYTES;
            let start = root.final_record.offset - base;
            let end = start + record.len();
            if page.after.get(start..end) != Some(root.final_record.data.as_slice()) {
                return Err(EditError::Preservation);
            }
            page.after[start..end].copy_from_slice(record);
            target = base + start..base + end;
            appended = None;
        }
        budget
            .charge_allocation(1, size_of::<Event>() as u64)
            .map_err(InspectError::from)?;
        self.events
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.events.push(Event {
            root: root.reference,
            word: root.word,
            root_range: root.range.clone(),
            target: target.clone(),
            appended,
        });
        Ok(target)
    }

    fn audit(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        super::audit::pages(self.view(), budget, cancelled)
    }

    fn target(
        &self,
        range: Range<usize>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&[u8], EditError> {
        self.view().target(range, budget, cancelled)
    }
}
