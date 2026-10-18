use super::bytes::{u16_at, u32_at};
use crate::{Header, InspectError, header::check};
use std::io::{Read, Seek, SeekFrom};
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
    pub fn page(&mut self, number: u32) -> Result<[u8; 4096], InspectError> {
        self.tick()?;
        if number >= self.header.page_count() {
            return Err(InspectError::Malformed);
        }
        if self.reads >= 4096 {
            return Err(InspectError::StructureLimit);
        }
        self.reads += 1;
        self.budget.charge(Resource::Bytes, 4096)?;
        self.source
            .seek(SeekFrom::Start(u64::from(number) * 4096))?;
        let mut page = [0; 4096];
        self.source.read_exact(&mut page)?;
        Ok(page)
    }
    // Explicit references may point to relocated/deleted slots. Enumeration skips
    // deleted roots; following a live lookup uses this physical accessor.
    pub fn row(
        &mut self,
        mut page_number: u32,
        mut slot: u8,
        owner: Option<u32>,
    ) -> Result<Vec<u8>, InspectError> {
        let mut visited = [0_u32; 16];
        for depth in 0..16 {
            self.tick()?;
            let key = (page_number << 8) | u32::from(slot);
            if visited[..depth].contains(&key) {
                return Err(InspectError::Malformed);
            }
            visited[depth] = key;
            let page = self.page(page_number)?;
            if page[0..2] != [1, 1]
                || owner.is_some_and(|owner| u32_at(&page, 4).ok() != Some(owner))
            {
                return Err(InspectError::Malformed);
            }
            let (_, start, end) = row_range(&page, usize::from(slot))?;
            let flags = u16_at(&page, 14 + 2 * usize::from(slot))?;
            if flags & 0x4000 != 0 {
                let reference = u32_at(&page[start..end], 0)?;
                page_number = reference >> 8;
                slot = reference as u8;
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
    let count = usize::from(u16_at(page, 12)?);
    if count > 255 || 14 + 2 * count > page.len() {
        return Err(InspectError::Malformed);
    }
    Ok(count)
}
pub(crate) fn row_range(page: &[u8], slot: usize) -> Result<(u16, usize, usize), InspectError> {
    let count = row_count(page)?;
    if slot >= count {
        return Err(InspectError::Malformed);
    }
    let raw = u16_at(page, 14 + 2 * slot)?;
    if raw & 0x2000 != 0 {
        return Err(InspectError::Unsupported);
    }
    let start = usize::from(raw & 0x1fff);
    let end = if slot == 0 {
        page.len()
    } else {
        usize::from(u16_at(page, 12 + 2 * slot)? & 0x1fff)
    };
    if start < 14 + 2 * count || start >= end || end > page.len() {
        return Err(InspectError::Malformed);
    }
    Ok((raw, start, end))
}
