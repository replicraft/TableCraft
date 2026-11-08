//! Existing DATA-page packing with stable logical slots and retained free bytes.
use super::{NativeTableRenameIdentity, TableRenameError};
use crate::{
    InspectError, NativeImage,
    format::{
        bytes::u16_at,
        page::{DATA_SIGNATURE, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{OFFSET_MASK, SLOT_BYTES, slot_offset},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_BYTES_OFFSET: usize = DATA_SIGNATURE.len();
const FREE_BYTES_END: usize = FREE_BYTES_OFFSET + size_of::<u16>();

pub(super) fn replace(
    image: &NativeImage,
    after: &mut [u8],
    identity: NativeTableRenameIdentity,
    row: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    check(budget, cancelled)?;
    let page_start = identity.catalog_page() as usize * PAGE_BYTES;
    let original = &image.as_bytes()[page_start..page_start + PAGE_BYTES];
    let candidate = &mut after[page_start..page_start + PAGE_BYTES];
    let count = row_count(original)?;
    let selected = usize::from(identity.catalog_slot());
    let (_, selected_start, selected_end) = row_range(original, selected)?;
    let old_length = selected_end - selected_start;
    let (_, low, _) = row_range(original, count - 1)?;
    let slots_end = slot_offset(count);
    let free = usize::from(u16_at(original, FREE_BYTES_OFFSET)?);
    if low.checked_sub(slots_end) != Some(free) {
        return Err(TableRenameError::Source);
    }
    let new_free = free
        .checked_add(old_length)
        .and_then(|free| free.checked_sub(row.len()))
        .ok_or(TableRenameError::Capacity)?;
    let new_low = slots_end
        .checked_add(new_free)
        .ok_or(TableRenameError::Capacity)?;
    if new_low > PAGE_BYTES {
        return Err(TableRenameError::Capacity);
    }
    for slot in selected..count {
        check(budget, cancelled)?;
        let (flags, start, end) = row_range(original, slot)?;
        if flags & !OFFSET_MASK != 0 {
            return Err(TableRenameError::Source);
        }
        let shifted = start
            .checked_add(old_length)
            .and_then(|start| start.checked_sub(row.len()))
            .ok_or(TableRenameError::Capacity)?;
        let data = if slot == selected {
            row
        } else {
            &original[start..end]
        };
        let destination_end = shifted
            .checked_add(data.len())
            .ok_or(TableRenameError::Capacity)?;
        if shifted < slots_end || destination_end > PAGE_BYTES {
            return Err(TableRenameError::Capacity);
        }
        budget
            .charge(Resource::Steps, data.len() as u64)
            .map_err(InspectError::from)?;
        candidate[shifted..destination_end].copy_from_slice(data);
        let at = slot_offset(slot);
        let offset = u16::try_from(shifted).map_err(|_| TableRenameError::Capacity)?;
        candidate[at..at + SLOT_BYTES].copy_from_slice(&offset.to_le_bytes());
    }
    let free = u16::try_from(new_free).map_err(|_| TableRenameError::Capacity)?;
    candidate[FREE_BYTES_OFFSET..FREE_BYTES_END].copy_from_slice(&free.to_le_bytes());

    if row_count(candidate)? != count {
        return Err(TableRenameError::Preservation);
    }
    for slot in 0..count {
        check(budget, cancelled)?;
        let (_, old_start, old_end) = row_range(original, slot)?;
        let (flags, new_start, new_end) = row_range(candidate, slot)?;
        if flags & !OFFSET_MASK != 0 {
            return Err(TableRenameError::Preservation);
        }
        let expected = if slot == selected {
            row
        } else {
            &original[old_start..old_end]
        };
        if candidate[new_start..new_end] != *expected {
            return Err(TableRenameError::Preservation);
        }
    }
    // All bytes outside the necessary row movement, slot words and free counter
    // retain their original values. Shrinkage deliberately keeps vacated bytes.
    let changed_start = low.min(new_low);
    let slot_start = slot_offset(selected);
    for offset in 0..PAGE_BYTES {
        check(budget, cancelled)?;
        let allowed = (FREE_BYTES_OFFSET..FREE_BYTES_END).contains(&offset)
            || (slot_start..slots_end).contains(&offset)
            || (changed_start..selected_end).contains(&offset);
        if !allowed && original[offset] != candidate[offset] {
            return Err(TableRenameError::Preservation);
        }
    }
    if new_low > low && original[low..new_low] != candidate[low..new_low] {
        return Err(TableRenameError::Preservation);
    }
    Ok(())
}
