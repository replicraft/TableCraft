//! Construct new leaf pages and a retained-root node using existing key codecs.
use super::super::{encoding, profile::SingleProfile, repack};
use crate::{
    EditError, InspectError,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::{Entry, KeyRef, LEAF, NODE},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const VERSION_OFFSET: usize = std::mem::offset_of!(IndexHeader, version);
const OWNER_OFFSET: usize = std::mem::offset_of!(IndexHeader, owner);
const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);

pub(super) fn leaf(
    owner: u32,
    previous: u32,
    next: u32,
    entries: &[Entry],
    profile: &SingleProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut initial = Vec::new();
    initial
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    initial.resize(PAGE_BYTES, 0);
    initial[0] = LEAF;
    initial[VERSION_OFFSET] = 1;
    initial[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()].copy_from_slice(&owner.to_le_bytes());
    initial[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
        .copy_from_slice(&previous.to_le_bytes());
    initial[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
    repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(&initial, entries, profile, budget, cancelled)
}

pub(super) fn root(
    original: &[u8],
    left: u32,
    right: u32,
    maximum: KeyRef,
    profile: &SingleProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut initial = Vec::new();
    initial
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    initial.extend_from_slice(original);
    initial[0] = NODE;
    initial[LEVEL_OFFSET] = 1;
    initial[TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()].copy_from_slice(&right.to_le_bytes());
    initial[PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()].fill(0);
    repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
        &initial,
        &[Entry {
            value: maximum,
            child: left,
        }],
        profile,
        budget,
        cancelled,
    )
}
