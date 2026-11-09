//! Checked dense DATA-page append without relocating a single existing record.
use crate::{
    InspectError,
    format::{
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row::slot_offset,
    },
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum PageError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("dense native page source is unsupported")]
    Source,
    #[error("dense native page append capacity is exhausted")]
    Capacity,
}

const HEADER_BYTES: usize = slot_offset(0);
const SLOT_BYTES: usize = size_of::<u16>();
const MAX_SLOTS: usize = u8::MAX as usize + 1;

pub(super) struct Page {
    pub count: usize,
    pub start: usize,
    pub records: Vec<Range<usize>>,
}

pub(super) fn word(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(
        raw[at..at + SLOT_BYTES].try_into().unwrap(),
    ))
}

impl Page {
    pub fn capture(
        raw: &[u8],
        owner: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PageError> {
        check(budget, cancelled)?;
        if raw.len() != PAGE_BYTES
            || raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || raw[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()] != owner.to_le_bytes()
            || raw[8..12] != [0; 4]
        {
            return Err(PageError::Source);
        }
        let count = word(raw, 12);
        if count > MAX_SLOTS {
            return Err(PageError::Source);
        }
        budget
            .charge_allocation(count as u64, size_of::<Range<usize>>() as u64)
            .map_err(InspectError::from)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        let directory = HEADER_BYTES + count * SLOT_BYTES;
        let mut end = PAGE_BYTES;
        for slot in 0..count {
            check(budget, cancelled)?;
            let start = word(raw, HEADER_BYTES + slot * SLOT_BYTES);
            if start < directory || start >= end {
                return Err(PageError::Source);
            }
            records.push(start..end);
            end = start;
        }
        if word(raw, 2) != end - directory {
            return Err(PageError::Source);
        }
        Ok(Self {
            count,
            start: end,
            records,
        })
    }

    pub fn append(&mut self, raw: &mut [u8], length: usize) -> Result<Range<usize>, PageError> {
        let start = self.start.checked_sub(length).ok_or(PageError::Capacity)?;
        let count = self
            .count
            .checked_add(1)
            .filter(|count| *count <= MAX_SLOTS)
            .ok_or(PageError::Capacity)?;
        let directory = HEADER_BYTES + count * SLOT_BYTES;
        if start < directory {
            return Err(PageError::Capacity);
        }
        let range = start..self.start;
        // NO_CLONE: Zero the newly allocated record span and return those same bounds as
        // the placement result. Only range bounds are copied, not the payload.
        raw[range.clone()].fill(0);
        let slot = HEADER_BYTES + self.count * SLOT_BYTES;
        raw[slot..slot + SLOT_BYTES].copy_from_slice(&(start as u16).to_le_bytes());
        raw[12..14].copy_from_slice(&(count as u16).to_le_bytes());
        raw[2..4].copy_from_slice(&((start - directory) as u16).to_le_bytes());
        self.count = count;
        self.start = start;
        Ok(range)
    }

    pub fn charge_append(
        length: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), PageError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (length + 3 * SLOT_BYTES) as u64)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        Ok(())
    }
}
