use super::bytes::{u16_at, u32_at};
use super::{
    page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
    row::*,
};
use crate::profile::{MAX_PAGE_READS, MAX_RELOCATION_HOPS};
use crate::{Header, InspectError, header::check};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) struct Reader<'a, R> {
    pub source: &'a mut R,
    pub header: Header,
    pub budget: &'a mut ResourceBudget,
    pub cancelled: &'a dyn Fn() -> bool,
    pub reads: u32,
}
impl<R: Read + Seek> Reader<'_, R> {
    pub fn tick(&mut self) -> Result<(), InspectError> {
        check(self.budget, self.cancelled)
    }
    pub fn page(&mut self, number: u32) -> Result<[u8; PAGE_BYTES], InspectError> {
        self.tick()?;
        if number >= self.header.page_count() {
            return Err(InspectError::Malformed);
        }
        if self.reads >= MAX_PAGE_READS {
            return Err(InspectError::StructureLimit);
        }
        self.reads += 1;
        self.budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
        let mut page = [0; PAGE_BYTES];
        tablecraft_io::read_exact_at(
            self.source,
            u64::from(number) * PAGE_BYTES as u64,
            &mut page,
            self.budget,
            self.cancelled,
        )?;
        Ok(page)
    }
    // Explicit references may point to relocated/deleted slots. Enumeration skips
    // deleted roots; following a live lookup uses this physical accessor.
    pub fn row(
        &mut self,
        page_number: u32,
        slot: u8,
        owner: Option<u32>,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_impl(page_number, slot, owner, false)
    }
    /// An indexed reference must start at a live root slot. Relocation targets
    /// retain the existing physical accessor semantics without rereading the root.
    pub fn live_row(
        &mut self,
        page_number: u32,
        slot: u8,
        owner: u32,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_impl(page_number, slot, Some(owner), true)
    }
    fn row_impl(
        &mut self,
        mut page_number: u32,
        mut slot: u8,
        owner: Option<u32>,
        live_root: bool,
    ) -> Result<Vec<u8>, InspectError> {
        let mut visited = [0_u32; MAX_RELOCATION_HOPS];
        for depth in 0..MAX_RELOCATION_HOPS {
            self.tick()?;
            let key = reference(page_number, slot);
            if visited[..depth].contains(&key) {
                return Err(InspectError::Malformed);
            }
            visited[depth] = key;
            let page = self.page(page_number)?;
            if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || owner.is_some_and(|owner| u32_at(&page, OWNER_OFFSET).ok() != Some(owner))
            {
                return Err(InspectError::Malformed);
            }
            let (flags, start, end) = row_range(&page, usize::from(slot))?;
            if depth == 0 && live_root && flags & DELETED_MASK != 0 {
                return Err(InspectError::Malformed);
            }
            if flags & RELOCATED_MASK != 0 {
                let reference = u32_at(&page[start..end], 0)?;
                page_number = reference_page(reference);
                slot = reference_slot(reference);
                continue;
            }
            self.budget.charge(Resource::Bytes, (end - start) as u64)?;
            let mut row = Vec::new();
            row.try_reserve_exact(end - start)
                .map_err(|_| InspectError::Allocation)?;
            row.extend_from_slice(&page[start..end]);
            return Ok(row);
        }
        Err(InspectError::StructureLimit)
    }
}
pub(crate) fn row_count(page: &[u8]) -> Result<usize, InspectError> {
    let count = usize::from(u16_at(page, ROW_COUNT_OFFSET)?);
    if count > MAX_SLOTS || slot_offset(count) > page.len() {
        return Err(InspectError::Malformed);
    }
    Ok(count)
}
pub(crate) fn row_range(page: &[u8], slot: usize) -> Result<(u16, usize, usize), InspectError> {
    let count = row_count(page)?;
    if slot >= count {
        return Err(InspectError::Malformed);
    }
    let raw = u16_at(page, slot_offset(slot))?;
    if raw & UNSUPPORTED_MASK != 0 {
        return Err(InspectError::Unsupported);
    }
    let start = usize::from(raw & OFFSET_MASK);
    let end = if slot == 0 {
        page.len()
    } else {
        usize::from(u16_at(page, slot_offset(slot - 1))? & OFFSET_MASK)
    };
    if start < slot_offset(count) || start >= end || end > page.len() {
        return Err(InspectError::Malformed);
    }
    Ok((raw, start, end))
}
