//! Complete direct DATA page framing; skipped/deleted slots are not absence.
use super::refused;
use crate::{
    EditError, InspectError,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        row::{OFFSET_MASK, ROW_COUNT_OFFSET, slot_offset},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const RESERVED_START: usize = OWNER_OFFSET + size_of::<u32>();
const SLOT_BYTES: usize = size_of::<u16>();
// This first closure profile admits 255 direct slots. This is deliberately a
// qualification ceiling, not a claim about the full native reference range.
const MAX_QUALIFIED_SLOTS: usize = u8::MAX as usize;

pub(super) fn count(
    page: &[u8],
    owner: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    if page.len() != PAGE_BYTES
        || page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || u32_at(page, OWNER_OFFSET)? != owner
        || page[RESERVED_START..ROW_COUNT_OFFSET]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(refused());
    }
    let count = usize::from(u16_at(page, ROW_COUNT_OFFSET)?);
    if count > MAX_QUALIFIED_SLOTS {
        return Err(refused());
    }
    let directory_end = slot_offset(count);
    let mut end = PAGE_BYTES;
    for ordinal in 0..count {
        check(budget, cancelled)?;
        let word = u16_at(page, slot_offset(ordinal))?;
        if word & !OFFSET_MASK != 0 {
            return Err(refused());
        }
        let start = usize::from(word);
        if start < directory_end || start >= end {
            return Err(refused());
        }
        end = start;
    }
    if end < directory_end || usize::from(u16_at(page, FREE_OFFSET)?) != end - directory_end {
        return Err(refused());
    }
    budget
        .charge(
            Resource::Steps,
            (end - directory_end + count * SLOT_BYTES) as u64,
        )
        .map_err(InspectError::from)?;
    if page[directory_end..end].iter().any(|byte| *byte != 0) {
        return Err(refused());
    }
    Ok(count)
}

pub(super) fn row(page: &[u8], ordinal: usize) -> Result<&[u8], EditError> {
    let start = usize::from(u16_at(page, slot_offset(ordinal))?);
    let end = if ordinal == 0 {
        PAGE_BYTES
    } else {
        usize::from(u16_at(page, slot_offset(ordinal - 1))?)
    };
    page.get(start..end).ok_or_else(refused)
}
