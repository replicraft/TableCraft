//! Current table DROP retains row payloads and every byte outside its proven actions.
use super::{TableDropError, row_effects};
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
        row::{DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, slot_offset},
    },
    header::check,
    table::mutation::drop_table::{source::Qualified, verify},
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

const TOMBSTONE: u16 = DELETED_MASK | RELOCATED_MASK;

struct CatalogAction {
    slot: Range<usize>,
    count: Range<usize>,
    word: u16,
    live: u32,
}

impl CatalogAction {
    fn capture(
        image: &NativeImage,
        source: &Qualified,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableDropError> {
        let slot = source.identity.catalog_page() as usize * PAGE_BYTES
            + slot_offset(usize::from(source.identity.catalog_slot()));
        let count = source.catalog_definition as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
        let word = u16_at(image.as_bytes(), slot)?;
        let live = u32_at(image.as_bytes(), count)?;
        if word & !(OFFSET_MASK | RELOCATED_MASK) != 0 || live == 0 || live != source.catalog_rows {
            return Err(TableDropError::Source);
        }
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let page = reader.page(source.identity.catalog_page())?;
        reader.row_location(
            source.identity.catalog_page(),
            source.identity.catalog_slot(),
            source.catalog_definition,
            &page,
        )?;
        Ok(Self {
            slot: slot..slot + size_of::<u16>(),
            count: count..count + size_of::<u32>(),
            word,
            live,
        })
    }

    fn contains(&self, offset: usize) -> bool {
        self.slot.contains(&offset) || self.count.contains(&offset)
    }
}

pub(in crate::table::mutation::drop_table) fn compose(
    before: &NativeImage,
    source: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TableDropError> {
    let mut after = row_effects(before, &source.table, budget, cancelled)?;
    let action = CatalogAction::capture(before, source, budget, cancelled)?;
    budget.charge(
        Resource::Steps,
        (size_of::<u16>() + size_of::<u32>()) as u64,
    )?;
    after
        .get_mut(action.slot.start..action.slot.end)
        .ok_or(InspectError::Truncated)?
        .copy_from_slice(&(action.word | TOMBSTONE).to_le_bytes());
    after
        .get_mut(action.count.start..action.count.end)
        .ok_or(InspectError::Truncated)?
        .copy_from_slice(&(action.live - 1).to_le_bytes());
    candidate(before, &after, source, budget, cancelled)?;
    Ok(after)
}

pub(in crate::table::mutation::drop_table) fn candidate(
    before: &NativeImage,
    after: &[u8],
    source: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableDropError> {
    check(budget, cancelled)?;
    let rows = row_effects(before, &source.table, budget, cancelled)?;
    let action = CatalogAction::capture(before, source, budget, cancelled)?;
    if after.len() != rows.len()
        || u16_at(after, action.slot.start)? != (action.word | TOMBSTONE)
        || u32_at(after, action.count.start)? != action.live - 1
    {
        return Err(TableDropError::Preservation);
    }
    for (page_index, (expected, observed)) in rows
        .chunks(PAGE_BYTES)
        .zip(after.chunks(PAGE_BYTES))
        .enumerate()
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, expected.len() as u64)?;
        for (position, (expected, observed)) in expected.iter().zip(observed).enumerate() {
            if expected != observed && !action.contains(page_index * PAGE_BYTES + position) {
                return Err(TableDropError::Preservation);
            }
        }
    }
    verify::catalog(before, after, source, budget, cancelled)
}
