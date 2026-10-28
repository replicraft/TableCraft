//! Cardinality-growing physical-index continuation; DATA authority stays with INSERT.
#[cfg(test)]
mod tests;
mod tree;

use super::super::{NumericKey, profile::SingleProfile, repack, tree::Tree};
use super::{AllocationPolicy, Plan, edit_prepared, leaf_entries, nonroot, seal};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, WriteBlocker,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::{
        indexes::{
            header::IndexHeader,
            page::{KeyRef, LEAF, NODE},
        },
        mutation::{
            append::{self, State},
            map_authority::{self, Owner},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

/// Final sorted keys must come from immutable INSERT requests and certified DATA
/// placements. This constructor does not certify new records or publish bytes.
pub(in crate::table::indexes) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub id: usize,
    pub physical: u32,
    pub final_keys: &'a [KeyRef],
}

/// Continue a qualified overflowing root leaf or recursive original-bound tree.
/// Existing fitted root-leaf INSERT remains a separate caller path. This
/// does not invoke UPDATE key preparation or the unchanged-cardinality arena.
pub(in crate::table::indexes) fn continue_primary(
    operation: Operation<'_>,
    append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let image = operation.image;
    let checked = operation.checked;
    let schema = checked.schema();
    let index = physical_index(&operation, budget, cancelled)?;
    if append.original_length != image.as_bytes().len()
        || append.owner != schema.definition_page()
        || append.length < append.original_length
        || append.length as u64 > MAX_IMAGE_BYTES
        || !append.length.is_multiple_of(PAGE_BYTES)
        || (append.length - append.original_length) / PAGE_BYTES != append.reservations_len()
        || append
            .indexes()
            .any(|prior| prior.physical >= operation.physical)
    {
        return Err(EditError::Preservation);
    }
    require_prior_owners(&operation, &append, budget, cancelled)?;
    append.preview(0)?;
    if operation.final_keys.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let profile = SingleProfile {
        column: schema.columns()[operation.id].number(),
        kind: NumericKey::new(
            schema.columns()[operation.id]
                .scalar_type()
                .ok_or(InspectError::Unsupported)?,
        )?,
        descending: index.columns()[0].descending(),
    };
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let offset = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let original = image
        .as_bytes()
        .get(offset..offset + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    let header = IndexHeader::read(original)?;
    if header.kind == NODE {
        return tree::continue_primary(operation, append, reader.budget, cancelled);
    }
    if header.kind != LEAF
        || header.previous.get() != 0
        || header.next.get() != 0
        || header.tail.get() != 0
        || !header.qualified_for_update(0)
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut original_keys = Vec::new();
    let tree = Tree::read::<_, ENTRY_BUFFER>(
        image,
        &mut reader,
        schema.definition_page(),
        index.root_page(),
        &profile,
        0,
        &mut original_keys,
    )?;
    if !tree.is_leaf() || original_keys.len() != checked.rows() {
        return Err(EditError::Preservation);
    }
    retained_members(
        &original_keys,
        operation.final_keys,
        &profile,
        index,
        &append,
        &mut reader,
    )?;
    let entries = leaf_entries(operation.final_keys, reader.budget, cancelled)?;
    if repack::borrowed_fits::<_, ENTRY_BUFFER>(
        original,
        &entries,
        &profile,
        reader.budget,
        cancelled,
    )? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let middle = entries.len() / 2;
    for half in [&entries[..middle], &entries[middle..]] {
        if !repack::borrowed_fits_fresh::<_, ENTRY_BUFFER>(
            half,
            &profile,
            reader.budget,
            cancelled,
        )? {
            return tree::continue_primary(operation, append, reader.budget, cancelled);
        }
    }
    // Every role possibly changed by DATA plus the selected physical index is
    // selected explicitly. Canonical aliases remain one physical owner.
    let definition = schema.definition_page();
    let census =
        map_authority::capture_catalog(image, reader.budget, cancelled, |_, _, _, _| Ok(()))?;
    census.require_exclusive(
        &[
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition },
            Owner::OrdinaryCandidate { definition },
            Owner::IndexOwned {
                definition,
                physical: index.data_number(),
            },
        ],
        reader.budget,
        cancelled,
    )?;
    prior_allocations(&append, &mut reader)?;

    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    let mut patches = Vec::new();
    edit_prepared(
        &nonroot::Operation::<_, ENTRY_BUFFER> {
            image,
            index,
            profile: &profile,
            keys: operation.final_keys,
        },
        &tree,
        profile.column,
        &mut patches,
        &mut plan,
        &mut reader,
    )?;
    plan.patches = patches;
    seal(
        image,
        checked.indexes(),
        &mut plan,
        reader.budget,
        cancelled,
    )?;
    check(reader.budget, cancelled)?;
    Ok(plan)
}

fn physical_index<'a>(
    operation: &Operation<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    super::super::insertion::qualify(
        operation.checked,
        operation.id,
        operation.physical,
        budget,
        cancelled,
    )
}

fn retained_members(
    original: &[KeyRef],
    final_keys: &[KeyRef],
    profile: &SingleProfile,
    index: &NativeIndex,
    append: &State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if final_keys.len() <= original.len() {
        return Err(EditError::Conflict);
    }
    let mut position = 0;
    let mut previous: Option<&KeyRef> = None;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous_key = None;
    for (number, entry) in final_keys.iter().enumerate() {
        reader.tick()?;
        // Re-encoding validates width and NULL marker for the bound profile.
        super::super::encoding::retained_entry::<_, ENTRY_BUFFER>(
            &crate::table::indexes::page::Entry {
                value: *entry,
                child: 0,
            },
            profile,
        )?;
        if previous.is_some_and(|prior| !prior.compare_with(entry, profile).is_lt()) {
            return Err(EditError::Conflict);
        }
        constraints
            .check_tuple(&mut previous_key, entry.key, entry.key.is_none())
            .map_err(InspectError::from)?;
        previous = Some(entry);
        let page = reference_page(entry.row) as usize;
        if page == 0
            || page >= append.length / PAGE_BYTES
            || usize::from(reference_slot(entry.row)) >= MAX_SLOTS
        {
            return Err(EditError::Conflict);
        }
        reader.work(number)?;
        if final_keys[..number]
            .iter()
            .any(|prior| prior.row == entry.row)
        {
            return Err(EditError::Conflict);
        }
        if original.get(position) == Some(entry) {
            position += 1;
            continue;
        }
        if original
            .get(position)
            .is_some_and(|old| !old.compare_with(entry, profile).is_gt())
        {
            return Err(EditError::Conflict);
        }
        if page >= append.original_length / PAGE_BYTES {
            append.require_data(reference_page(entry.row), reader.budget, reader.cancelled)?;
        }
    }
    if position != original.len() {
        return Err(EditError::Conflict);
    }
    Ok(())
}

fn require_prior_owners(
    operation: &Operation<'_>,
    append: &State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for prior in append.indexes() {
        check(budget, cancelled)?;
        let mut matched = false;
        for item in operation.checked.indexes() {
            check(budget, cancelled)?;
            let index = item.index();
            matched |= index.data_number() == prior.physical && index.usage_map() == prior.usage;
        }
        if prior.physical >= operation.physical || !matched {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

fn prior_allocations(
    append: &State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(reader, &global)?;
    for number in append.original_length / PAGE_BYTES..append.length / PAGE_BYTES {
        reader.tick()?;
        let number = u32::try_from(number).map_err(|_| InspectError::StructureLimit)?;
        let cell = global.future_cell(reader, number)?;
        if cell.before & cell.mask == 0 || append::current(&append.map_edits, &cell, reader)? {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}
