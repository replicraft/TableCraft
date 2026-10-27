//! Separate, bounded root-leaf splitting; ordinary replacement paths stay closed.
mod audit;
mod maps;
mod nonroot;
mod pages;

use super::{IndexDataPatch, IndexKeyUpdate, encoding, plan::keys, profile::SingleProfile, repack};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::MAX_IMAGE_BYTES,
    table::indexes::page::{Entry, KeyRef},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct AddedPage {
    pub number: u32,
    pub bytes: Vec<u8>,
}

pub(super) struct Certificate {
    pub physical: u32,
    pub root: u32,
    pub usage: u32,
    pub column: usize,
    pub descending: bool,
    pub right: u32,
    pub left: u32,
    pub count: usize,
}

pub(in crate::table) struct Plan {
    pub original_length: usize,
    pub length: usize,
    pub patches: Vec<IndexDataPatch>,
    pub added: Vec<AddedPage>,
    pub map_edits: Vec<maps::Edit>,
    certificates: Vec<Certificate>,
    owner: u32,
}

impl Plan {
    pub(super) fn new(image: &NativeImage, owner: u32) -> Self {
        Self {
            original_length: image.as_bytes().len(),
            length: image.as_bytes().len(),
            patches: Vec::new(),
            added: Vec::new(),
            map_edits: Vec::new(),
            certificates: Vec::new(),
            owner,
        }
    }

    pub(in crate::table) fn audit(
        &self,
        image: &NativeImage,
        checked: &TableCheck,
        updates: &[IndexKeyUpdate],
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        audit::prove(image, checked, updates, self, after, budget, cancelled)
    }
}

pub(super) fn edit(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    updates: &[IndexKeyUpdate],
    profile: &SingleProfile,
    patches: &mut Vec<IndexDataPatch>,
    plan: &mut Plan,
) -> Result<(), EditError> {
    let owner = plan.owner;
    let prepared = keys::prepare::<_, { encoding::MAX_ENTRY_BYTES }>(
        image, reader, index, owner, updates, profile,
    )?;
    if !prepared.tree.is_leaf() {
        return nonroot::edit(
            &nonroot::Operation {
                image,
                index,
                profile,
            },
            &prepared.tree,
            &prepared.entries,
            plan,
            reader,
            patches,
        );
    }
    let entries = leaf_entries(&prepared.entries, reader.budget, reader.cancelled)?;
    let offset = index.root_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(offset..offset + PAGE_BYTES)
        .ok_or(InspectError::Malformed)?;
    if repack::fits::<_, { encoding::MAX_ENTRY_BYTES }>(
        original,
        &entries,
        profile,
        reader.budget,
        reader.cancelled,
    )? {
        let after = repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            &entries,
            profile,
            reader.budget,
            reader.cancelled,
        )?;
        return repack::patches(
            original,
            &after,
            index.root_page(),
            patches,
            reader.budget,
            reader.cancelled,
        );
    }
    if entries.len() < 2 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let length = plan
        .length
        .checked_add(2 * PAGE_BYTES)
        .filter(|&length| length as u64 <= MAX_IMAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let right =
        u32::try_from(plan.length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
    let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
    let fact = Certificate {
        physical: index.data_number(),
        root: index.root_page(),
        usage: index.usage_map(),
        column: profile.column,
        descending: profile.descending,
        right,
        left,
        count: entries.len(),
    };
    let middle = entries.len() / 2;
    let left_page = pages::leaf(
        owner,
        0,
        right,
        &entries[..middle],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    let right_page = pages::leaf(
        owner,
        left,
        0,
        &entries[middle..],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    let root = pages::root(
        original,
        left,
        right,
        entries[middle - 1].value,
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    maps::allocate(
        image,
        &fact,
        &mut plan.map_edits,
        reader.budget,
        reader.cancelled,
    )?;
    reader
        .budget
        .charge_allocation(1, size_of::<Certificate>() as u64)
        .map_err(InspectError::from)?;
    plan.certificates
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    reader
        .budget
        .charge_allocation(2, size_of::<AddedPage>() as u64)
        .map_err(InspectError::from)?;
    plan.added
        .try_reserve(2)
        .map_err(|_| InspectError::Allocation)?;
    // Physical allocation order is deliberately right, then left.
    plan.added.push(AddedPage {
        number: right,
        bytes: right_page,
    });
    plan.added.push(AddedPage {
        number: left,
        bytes: left_page,
    });
    plan.certificates.push(fact);
    plan.length = length;
    reader
        .budget
        .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    reader
        .budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    let mut before = Vec::new();
    before
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    before.extend_from_slice(original);
    patches.push(IndexDataPatch {
        offset,
        before,
        after: root,
    });
    Ok(())
}

fn leaf_entries(
    keys: &[KeyRef],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Entry>, EditError> {
    budget
        .charge_allocation(keys.len() as u64, size_of::<Entry>() as u64)
        .map_err(InspectError::from)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(keys.len())
        .map_err(|_| InspectError::Allocation)?;
    for value in keys {
        check(budget, cancelled)?;
        entries.push(Entry {
            value: *value,
            child: 0,
        });
    }
    Ok(entries)
}
