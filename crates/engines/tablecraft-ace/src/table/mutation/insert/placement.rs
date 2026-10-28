//! Visible ordinary row append using the working shared map allocator.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::row_count,
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, UNSUPPORTED_MASK,
            reference, slot_offset,
        },
    },
    header::check,
    table::{
        mutation::{
            append::State,
            variable_split::metadata::Metadata,
            variable_update::{maps::Maps, placement::Page},
        },
        record_layout,
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const EMPTY_FREE: usize = PAGE_BYTES - slot_offset(0);

/// Returned builder facts. The parent must rederive them from immutable requests.
pub(super) struct InsertedRow {
    pub reference: u32,
    pub range: Range<usize>,
}

pub(super) struct Plan {
    pub pages: Vec<Page>,
    pub maps: Maps,
    pub append: State,
    pub rows: Vec<InsertedRow>,
}

pub(super) fn prepare(
    image: &NativeImage,
    schema: &TableSchema,
    metadata: Metadata,
    records: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let mut maps = Maps::capture(image, schema, budget, cancelled)?;
    maps.enable_hint_growth(image, schema, &metadata, budget, cancelled)?;
    budget
        .charge_allocation(records.len() as u64, size_of::<InsertedRow>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(records.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut plan = Plan {
        pages: Vec::new(),
        maps,
        append: State::new(image, schema.definition_page()),
        rows,
    };
    for record in records {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        if record
            .len()
            .checked_add(SLOT_BYTES)
            .is_none_or(|needed| needed > EMPTY_FREE)
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let layout = record_layout::parse(record, schema, budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let position = plan.destination(image, &metadata, record.len(), budget, cancelled)?;
        let page = &mut plan.pages[position];
        let (count, end) = geometry(&page.after, budget, cancelled)?;
        let start = end
            .checked_sub(record.len())
            .filter(|start| *start >= slot_offset(count + 1))
            .ok_or(EditError::Preservation)?;
        if count >= MAX_SLOTS {
            return Err(EditError::Preservation);
        }
        let offset = (page.number as usize)
            .checked_mul(PAGE_BYTES)
            .and_then(|offset| offset.checked_add(start))
            .ok_or(InspectError::StructureLimit)?;
        let range_end = offset
            .checked_add(record.len())
            .ok_or(InspectError::StructureLimit)?;
        let word = u16::try_from(start).map_err(|_| InspectError::StructureLimit)?;
        if word & !OFFSET_MASK != 0 {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, (record.len() + 3 * SLOT_BYTES) as u64)
            .map_err(InspectError::from)?;
        check(budget, cancelled)?;
        page.after[start..end].copy_from_slice(record);
        page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&word.to_le_bytes());
        page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((count + 1) as u16).to_le_bytes());
        page.after[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((start - slot_offset(count + 1)) as u16).to_le_bytes());
        plan.rows.push(InsertedRow {
            reference: reference(page.number, count as u8),
            range: offset..range_end,
        });
    }
    for page in &plan.pages {
        check(budget, cancelled)?;
        if page.before.is_empty() {
            plan.append.require_data(page.number, budget, cancelled)?;
        }
    }
    for (number, reference, _) in plan.maps.metadata_pages() {
        plan.append.require_candidate_bitmap(
            number,
            schema.definition_page(),
            reference,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(plan)
}

impl Plan {
    fn destination(
        &mut self,
        image: &NativeImage,
        metadata: &Metadata,
        length: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        if let Some(number) = self
            .maps
            .candidate(image, &self.append, budget, cancelled)?
        {
            metadata.require_data_page(number, budget, cancelled)?;
            let position = self.page(image, number, budget, cancelled)?;
            let (count, end) = geometry(&self.pages[position].after, budget, cancelled)?;
            if count < MAX_SLOTS && length + SLOT_BYTES <= end - slot_offset(count) {
                return Ok(position);
            }
            self.maps
                .failed_fit(image, number, &mut self.append, budget, cancelled)?;
        }
        self.append.preview(1)?;
        let number = self.append.next()?;
        metadata.require_data_page(number, budget, cancelled)?;
        self.maps
            .allocate(image, number, &mut self.append, budget, cancelled)?;
        let mut initial = copied(&[0; PAGE_BYTES], budget, cancelled)?;
        initial[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        initial[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
        initial[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
            .copy_from_slice(&self.append.owner.to_le_bytes());
        self.push_page(number, &[], &initial, budget, cancelled)
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
        let start = (number as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Malformed)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(raw, OWNER_OFFSET)? != self.append.owner
        {
            return Err(InspectError::Malformed.into());
        }
        geometry(raw, budget, cancelled)?;
        self.push_page(number, raw, raw, budget, cancelled)
    }

    fn push_page(
        &mut self,
        number: u32,
        before: &[u8],
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, EditError> {
        budget
            .charge_allocation(1, size_of::<Page>() as u64)
            .map_err(InspectError::from)?;
        self.pages
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        let before = copied(before, budget, cancelled)?;
        let after = copied(after, budget, cancelled)?;
        let position = self.pages.len();
        self.pages.push(Page {
            number,
            before,
            after,
        });
        Ok(position)
    }
}

fn copied(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(bytes);
    check(budget, cancelled)?;
    Ok(result)
}

fn geometry(
    page: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), EditError> {
    check(budget, cancelled)?;
    if page.len() != PAGE_BYTES {
        return Err(InspectError::Malformed.into());
    }
    let count = row_count(page)?;
    let slots_end = slot_offset(count);
    let mut end = PAGE_BYTES;
    for slot in 0..count {
        check(budget, cancelled)?;
        let word = u16_at(page, slot_offset(slot))?;
        if word & UNSUPPORTED_MASK != 0 {
            return Err(InspectError::Unsupported.into());
        }
        let start = usize::from(word & OFFSET_MASK);
        if start < slots_end || start > end || (start == end && word & DELETED_MASK == 0) {
            return Err(InspectError::Malformed.into());
        }
        end = start;
    }
    if usize::from(u16_at(page, FREE_OFFSET)?) != end - slots_end {
        return Err(InspectError::Malformed.into());
    }
    Ok((count, end))
}

#[cfg(test)]
mod tests;
