//! Owned heterogeneous spans; only disjoint edits to one row-mask byte merge.
use super::super::super::{EditError, patch::BytePatch};
use crate::{InspectError, format::data_type::NUMERIC_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_FIXED_BYTES: usize = NUMERIC_BYTES;

pub(super) struct Patch {
    offset: usize,
    before: [u8; MAX_FIXED_BYTES],
    after: [u8; MAX_FIXED_BYTES],
    length: usize,
    mask: u8,
}
impl BytePatch for Patch {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before[..self.length]
    }
    fn after(&self) -> &[u8] {
        &self.after[..self.length]
    }
    fn is_mask(&self) -> bool {
        self.mask != 0
    }
}

pub(super) fn push<P: BytePatch>(
    patches: &mut Vec<Patch>,
    input: &P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let length = input.before().len();
    if length == 0
        || length > MAX_FIXED_BYTES
        || input.after().len() != length
        || input.is_mask() && length != 1
    {
        return Err(EditError::Preservation);
    }
    let mask = if input.is_mask() {
        input.before()[0] ^ input.after()[0]
    } else {
        0
    };
    if input.is_mask() && mask.count_ones() != 1 {
        return Err(EditError::Preservation);
    }
    budget
        .charge_allocation(1, size_of::<Patch>() as u64)
        .map_err(InspectError::from)?;
    patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    let mut before = [0; MAX_FIXED_BYTES];
    let mut after = [0; MAX_FIXED_BYTES];
    before[..length].copy_from_slice(input.before());
    after[..length].copy_from_slice(input.after());
    patches.push(Patch {
        offset: input.offset(),
        before,
        after,
        length,
        mask,
    });
    Ok(())
}

pub(super) fn coalesce(
    patches: &mut Vec<Patch>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = patches.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, (patches.len() * levels) as u64)
        .map_err(InspectError::from)?;
    patches.sort_unstable_by_key(|patch| patch.offset);
    let mut write = 0;
    for read in 0..patches.len() {
        check(budget, cancelled)?;
        if write > 0 && patches[write - 1].offset == patches[read].offset {
            let (before, after) = patches.split_at_mut(read);
            let prior = &mut before[write - 1];
            let next = &after[0];
            if prior.mask == 0
                || next.mask == 0
                || prior.before[0] != next.before[0]
                || prior.mask & next.mask != 0
            {
                return Err(EditError::Preservation);
            }
            prior.after[0] = (prior.after[0] & !next.mask) | (next.after[0] & next.mask);
            prior.mask |= next.mask;
        } else {
            if write > 0
                && patches[write - 1]
                    .offset
                    .checked_add(patches[write - 1].length)
                    .ok_or(EditError::Preservation)?
                    > patches[read].offset
            {
                return Err(EditError::Preservation);
            }
            patches.swap(write, read);
            write += 1;
        }
    }
    patches.truncate(write);
    check(budget, cancelled)?;
    Ok(())
}
