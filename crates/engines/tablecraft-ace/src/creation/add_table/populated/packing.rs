//! Ordered physical DATA packing derives limits from record and ROW geometry.
use crate::{
    InspectError,
    creation::{CreateDatabaseError, record::DATA_HEADER_BYTES},
    format::{page::PAGE_BYTES, row::SLOT_BYTES},
    header::check,
};
use std::ops::Range;
use tablecraft_core::limits::ResourceBudget;

const MAX_SLOTS: usize = u8::MAX as usize + 1;

pub(in crate::creation::add_table) struct Page {
    pub rows: Range<usize>,
    pub used: usize,
}

impl Page {
    pub fn candidate(&self, minimum_row: usize) -> bool {
        self.rows.end - self.rows.start < MAX_SLOTS
            && minimum_row
                .checked_add(SLOT_BYTES)
                .is_some_and(|needed| needed <= PAGE_BYTES - self.used)
    }
}

pub(super) fn capture(
    lengths: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Page>, CreateDatabaseError> {
    let mut pages = Vec::new();
    let mut start = 0;
    let mut used = DATA_HEADER_BYTES;
    for (row, &length) in lengths.iter().enumerate() {
        check(budget, cancelled)?;
        let bytes = length
            .checked_add(SLOT_BYTES)
            .ok_or(CreateDatabaseError::Layout)?;
        if bytes > PAGE_BYTES - DATA_HEADER_BYTES {
            return Err(CreateDatabaseError::PageCapacity);
        }
        if row - start == MAX_SLOTS || bytes > PAGE_BYTES - used {
            push(
                &mut pages,
                Page {
                    rows: start..row,
                    used,
                },
                budget,
            )?;
            start = row;
            used = DATA_HEADER_BYTES;
        }
        used += bytes;
    }
    push(
        &mut pages,
        Page {
            rows: start..lengths.len(),
            used,
        },
        budget,
    )?;
    Ok(pages)
}

fn push(
    pages: &mut Vec<Page>,
    page: Page,
    budget: &mut ResourceBudget,
) -> Result<(), InspectError> {
    if pages.len() == pages.capacity() {
        let added = pages.capacity().max(1);
        budget.charge_allocation(added as u64, size_of::<Page>() as u64)?;
        budget.charge(tablecraft_core::limits::Resource::Steps, pages.len() as u64)?;
        pages
            .try_reserve_exact(added)
            .map_err(|_| InspectError::Allocation)?;
    }
    pages.push(page);
    Ok(())
}
