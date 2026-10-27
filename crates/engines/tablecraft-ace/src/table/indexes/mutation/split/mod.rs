//! Separate, bounded recursive splitting; ordinary replacement paths stay closed.
mod allocation;
mod audit;
mod maps;
mod nonroot;
mod pages;
mod slot;

pub(in crate::table) use slot::{
    continue_patches as continue_slot_patches, patches as slot_patches, qualify as qualify_slot,
};

use super::{
    IndexDataPatch, IndexKeyUpdate, encoding, plan::keys, profile::SingleProfile, repack,
    replacement::SlotKeyChanges, retained::RetainedProfile, tree::Tree,
};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::{
        indexes::page::{Entry, KeyRef},
        mutation::{CertifiedData, append::State},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(in crate::table) fn supported_scalar(kind: ScalarType) -> bool {
    matches!(
        kind,
        ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64
            | ScalarType::Currency
    )
}

pub(in crate::table) struct AddedPage {
    pub number: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(super) enum AllocationPolicy {
    Legacy,
    Qualified,
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
    pub append: State,
    pub patches: Vec<IndexDataPatch>,
    pub added: Vec<AddedPage>,
    certificates: Vec<Certificate>,
    pub(in crate::table) promotions: Vec<allocation::Promotion>,
    pub(in crate::table) rebases: Vec<allocation::Rebase>,
    allocation_policy: AllocationPolicy,
}

impl Plan {
    pub(super) fn new(image: &NativeImage, owner: u32) -> Self {
        Self::with_append(State::new(image, owner))
    }

    pub(super) fn with_append(append: State) -> Self {
        Self::with_policy(append, AllocationPolicy::Legacy)
    }

    pub(super) fn with_policy(append: State, allocation_policy: AllocationPolicy) -> Self {
        Self {
            append,
            patches: Vec::new(),
            added: Vec::new(),
            certificates: Vec::new(),
            promotions: Vec::new(),
            rebases: Vec::new(),
            allocation_policy,
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

    pub(in crate::table) fn audit_slot(
        &self,
        image: &NativeImage,
        checked: &TableCheck,
        changes: &SlotKeyChanges<'_>,
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        audit::prove_slot(image, checked, changes, self, after, budget, cancelled)
    }

    pub(in crate::table) fn audit_slot_after_data(
        &self,
        image: &NativeImage,
        data: &CertifiedData<'_>,
        after: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        audit::prove_slot_after_data(image, data, self, after, budget, cancelled)
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
    let owner = plan.append.owner;
    let prepared = keys::prepare::<_, { encoding::MAX_ENTRY_BYTES }>(
        image, reader, index, owner, updates, profile,
    )?;
    edit_prepared(
        &nonroot::Operation::<_, { encoding::MAX_ENTRY_BYTES }> {
            image,
            index,
            profile,
            keys: &prepared.entries,
        },
        &prepared.tree,
        profile.column,
        patches,
        plan,
        reader,
    )
}

/// Physical construction consumes already qualified, immutable final keys.
fn edit_prepared<P: RetainedProfile, const N: usize>(
    operation: &nonroot::Operation<'_, P, N>,
    tree: &Tree,
    column: usize,
    patches: &mut Vec<IndexDataPatch>,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let image = operation.image;
    let index = operation.index;
    let profile = operation.profile;
    let owner = plan.append.owner;

    if !tree.is_leaf() {
        return nonroot::edit(operation, tree, plan, reader, patches);
    }
    let entries = leaf_entries(operation.keys, reader.budget, reader.cancelled)?;
    let offset = index.root_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(offset..offset + PAGE_BYTES)
        .ok_or(InspectError::Malformed)?;
    if repack::borrowed_fits::<P, N>(original, &entries, profile, reader.budget, reader.cancelled)?
    {
        let after = repack::borrowed_page::<P, N>(
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
    let middle = entries.len() / 2;
    let left_fits = repack::borrowed_fits_fresh::<P, N>(
        &entries[..middle],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    let right_fits = repack::borrowed_fits_fresh::<P, N>(
        &entries[middle..],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    if !left_fits || !right_fits {
        return nonroot::edit(operation, tree, plan, reader, patches);
    }
    let right = allocation::reserve(image, index, plan, reader)?;
    let left = allocation::reserve(image, index, plan, reader)?;
    let fact = Certificate {
        physical: index.data_number(),
        root: index.root_page(),
        usage: index.usage_map(),
        column,
        descending: index
            .columns()
            .first()
            .ok_or(InspectError::Malformed)?
            .descending(),
        right,
        left,
        count: entries.len(),
    };
    let left_page = pages::leaf::<P, N>(
        owner,
        0,
        right,
        &entries[..middle],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    let right_page = pages::leaf::<P, N>(
        owner,
        left,
        0,
        &entries[middle..],
        profile,
        reader.budget,
        reader.cancelled,
    )?;
    let root = pages::root::<P, N>(
        original,
        left,
        right,
        entries[middle - 1].value,
        profile,
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

fn borrowed_reference<K>(key: &KeyRef<K>) -> KeyRef<&K> {
    KeyRef {
        key: &key.key,
        row: key.row,
    }
}

fn leaf_entries<'a, K>(
    keys: &'a [KeyRef<K>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Entry<&'a K>>, EditError> {
    budget
        .charge_allocation(keys.len() as u64, size_of::<Entry<&K>>() as u64)
        .map_err(InspectError::from)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(keys.len())
        .map_err(|_| InspectError::Allocation)?;
    for value in keys {
        check(budget, cancelled)?;
        entries.push(Entry {
            value: borrowed_reference(value),
            child: 0,
        });
    }
    Ok(entries)
}

pub(super) fn seal(
    image: &NativeImage,
    indexes: &[IndexCheck],
    plan: &mut Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut source = image.reader();
    allocation::seal(
        image,
        indexes,
        plan,
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        },
    )
}
