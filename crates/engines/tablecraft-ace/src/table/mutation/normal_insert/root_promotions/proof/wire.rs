//! Manual frame replay from original seeds; no shared page serializer is called.
use super::keys::{self, Key};
use crate::{
    EditError, InspectError,
    format::{bytes::u16_at, page::PAGE_BYTES},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MASK_AT: usize = 27;
const ENTRIES: usize = 480;
const MASK_BYTES: usize = ENTRIES - MASK_AT;
const MAX_WIRE: usize = 13;

pub(super) fn frame(
    page: &mut [u8],
    keys: &[Key],
    profile: (bool, bool),
    prefix: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let (wide, descending) = profile;
    if page.len() != PAGE_BYTES || prefix >= if wide { 13 } else { 9 } {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    page[MASK_AT..ENTRIES].fill(0);
    let first = keys
        .first()
        .copied()
        .map(|key| keys::wire(key, wide, descending));
    let mut cursor = 0_usize;
    for (ordinal, key) in keys.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (3 * MAX_WIRE) as u64)
            .map_err(InspectError::from)?;
        let (raw, length) = keys::wire(*key, wide, descending);
        let shared = if ordinal == 0 { 0 } else { prefix };
        if first.as_ref().ok_or(EditError::Preservation)?.0[..shared] != raw[..shared] {
            return Err(EditError::Preservation);
        }
        let end = cursor
            .checked_add(length - shared)
            .filter(|end| *end <= PAGE_BYTES - ENTRIES)
            .ok_or(EditError::Preservation)?;
        page[ENTRIES + cursor..ENTRIES + end].copy_from_slice(&raw[shared..length]);
        page[MASK_AT + end / 8] |= 1 << (end % 8);
        cursor = end;
    }
    budget
        .charge(Resource::Steps, (2 * size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    page[2..4].copy_from_slice(&((PAGE_BYTES - ENTRIES - cursor) as u16).to_le_bytes());
    page[24..26].copy_from_slice(&(prefix as u16).to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn require_original(
    page: &[u8],
    keys: &[Key],
    profile: (bool, bool),
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let prefix = usize::from(u16_at(page, 24)?);
    let mut replay = crate::table::mutation::fixed_records::copy(page, budget, cancelled)?;
    frame(&mut replay, keys, profile, prefix, budget, cancelled)?;
    crate::table::mutation::normal_insert::compare(&replay, page, budget, cancelled)
}

pub(super) fn leaves(
    owner: u32,
    pages: (u32, u32),
    keys: &[Key],
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[Vec<u8>; 2], EditError> {
    let (right, left) = pages;
    let middle = keys.len() / 2;
    if middle == 0 {
        return Err(EditError::Preservation);
    }
    let mut result = [
        super::super::vector(PAGE_BYTES, budget, cancelled)?,
        super::super::vector(PAGE_BYTES, budget, cancelled)?,
    ];
    for (position, page) in result.iter_mut().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (PAGE_BYTES + 4 * size_of::<u32>()) as u64)
            .map_err(InspectError::from)?;
        page.resize(PAGE_BYTES, 0);
        page[..2].copy_from_slice(&[4, 1]);
        page[4..8].copy_from_slice(&owner.to_le_bytes());
        let (previous, next, population) = if position == 0 {
            (left, 0, &keys[middle..])
        } else {
            (0, right, &keys[..middle])
        };
        page[12..16].copy_from_slice(&previous.to_le_bytes());
        page[16..20].copy_from_slice(&next.to_le_bytes());
        frame(page, population, (true, descending), 0, budget, cancelled)?;
    }
    Ok(result)
}

pub(super) fn parent(
    page: &mut [u8],
    pages: (u32, u32),
    maximum: Key,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let (right, left) = pages;
    budget
        .charge(Resource::Steps, (MASK_BYTES + 2 * MAX_WIRE + 32) as u64)
        .map_err(InspectError::from)?;
    page[0] = 3;
    page[26] = 1;
    page[20..24].copy_from_slice(&right.to_le_bytes());
    page[24..26].fill(0);
    page[2..4].copy_from_slice(&3599_u16.to_le_bytes());
    page[MASK_AT..ENTRIES].fill(0);
    page[MASK_AT + 17 / 8] = 1 << (17 % 8);
    let (wire, length) = keys::wire(maximum, true, descending);
    if length != MAX_WIRE {
        return Err(EditError::Preservation);
    }
    page[ENTRIES..ENTRIES + MAX_WIRE].copy_from_slice(&wire);
    page[ENTRIES + MAX_WIRE..ENTRIES + MAX_WIRE + 4].copy_from_slice(&left.to_be_bytes());
    Ok(())
}
