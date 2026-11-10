//! Logical declaration storage and its complete checked physical footprint.
use super::super::refused;
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::{self, Definition, NEXT_PAGE_OFFSET, PAGE_HEADER_BYTES},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
};
use std::ops::Deref;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = size_of::<u16>();

pub(in crate::table::mutation::ordinary_content) struct Storage {
    root: u32,
    logical: Definition,
    continuations: Vec<u32>,
}

impl Deref for Storage {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.logical
    }
}

impl Storage {
    pub(super) fn pages(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(self.root).chain(self.continuations.iter().copied())
    }

    pub(super) fn joined(&self) -> bool {
        !self.continuations.is_empty()
    }

    pub(super) fn root_free_bytes(&self) -> usize {
        PAGE_BYTES.saturating_sub(self.len().min(PAGE_BYTES) + PAGE_HEADER_BYTES)
    }

    pub(super) fn validate_frames(
        &self,
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        let mut remaining = self.len();
        for (index, number) in self.pages().enumerate() {
            check(budget, cancelled)?;
            let page = physical_page(image, number)?;
            let header = if index == 0 { 0 } else { PAGE_HEADER_BYTES };
            let take = remaining.min(PAGE_BYTES - header);
            // Native TDEF free-space accounting reserves another page-header
            // width after the occupied prefix, including continuation headers.
            let free = PAGE_BYTES.saturating_sub(header + take + PAGE_HEADER_BYTES);
            if usize::from(u16_at(page, FREE_OFFSET)?) != free
                || u32_at(page, NEXT_PAGE_OFFSET)?
                    != self.continuations.get(index).copied().unwrap_or(0)
            {
                return Err(refused());
            }
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            // Joined declarations have an exact zero-padded constructive frame.
            // The existing one-page retired-tail policy is checked by raw.rs.
            if self.joined() && page[header + take..].iter().any(|byte| *byte != 0) {
                return Err(refused());
            }
            remaining -= take;
        }
        if remaining != 0 {
            return Err(refused());
        }
        Ok(())
    }
}

pub(in crate::table::mutation::ordinary_content) fn read(
    image: &NativeImage,
    root: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Storage, EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut continuations = Vec::new();
    let mut first = true;
    let logical = definition::read_with_pages(&mut reader, root, |number, reader| {
        if first {
            first = false;
            if number != root {
                return Err(InspectError::Malformed);
            }
            return Ok(());
        }
        if continuations.len() == continuations.capacity() {
            let growth = continuations.capacity().max(1);
            reader
                .budget
                .charge_allocation(growth as u64, size_of::<u32>() as u64)?;
            continuations
                .try_reserve_exact(growth)
                .map_err(|_| InspectError::Allocation)?;
        }
        continuations.push(number);
        Ok(())
    })?;
    Ok(Storage {
        root,
        logical,
        continuations,
    })
}

pub(super) fn physical_page(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let start = usize::try_from(number)
        .ok()
        .and_then(|number| number.checked_mul(PAGE_BYTES))
        .ok_or_else(refused)?;
    let end = start.checked_add(PAGE_BYTES).ok_or_else(refused)?;
    image.as_bytes().get(start..end).ok_or_else(refused)
}

#[cfg(test)]
mod tests;
