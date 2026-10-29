//! Cardinality-growing physical-index continuation; DATA authority stays with INSERT.
#[cfg(test)]
mod tests;
mod tree;

use super::super::{
    insertion::{binding::InsertProfile, membership},
    repack,
    retained::RetainedProfile,
    tree::Tree,
};
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
        mutation::{append::State, map_authority::Owner},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

#[cfg(test)]
const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

/// Final sorted keys must come from immutable INSERT requests and certified DATA
/// placements. This constructor does not certify new records or publish bytes.
pub(in crate::table::indexes::mutation) struct Operation<'a, P: RetainedProfile> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub index: &'a NativeIndex,
    pub profile: &'a P,
    pub final_keys: &'a [KeyRef<P::Key>],
}

/// Continue a qualified overflowing root leaf or recursive original-bound tree.
/// Existing fitted root-leaf INSERT remains a separate caller path. This
/// does not invoke UPDATE key preparation or the unchanged-cardinality arena.
pub(in crate::table::indexes::mutation) fn continue_index<P: InsertProfile, const N: usize>(
    operation: Operation<'_, P>,
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
            .any(|prior| prior.physical >= operation.index.data_number())
    {
        return Err(EditError::Preservation);
    }
    require_prior_owners(&operation, &append, budget, cancelled)?;
    append.preview(0)?;
    if operation.final_keys.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let profile = operation.profile;
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
        return tree::continue_index::<_, N>(operation, append, reader.budget, cancelled);
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
    let tree = Tree::read::<_, N>(
        image,
        &mut reader,
        schema.definition_page(),
        index.root_page(),
        profile,
        0,
        &mut original_keys,
    )?;
    profile.bind_source(checked.schema(), index, &mut original_keys, &mut reader)?;
    membership::require_population(index, &original_keys, profile, checked.rows(), &mut reader)?;
    if !tree.is_leaf() {
        return Err(EditError::Preservation);
    }
    retained_members::<_, N>(
        &original_keys,
        operation.final_keys,
        profile,
        index,
        &append,
        &mut reader,
    )?;
    let entries = leaf_entries(operation.final_keys, reader.budget, cancelled)?;
    if repack::borrowed_fits::<_, N>(original, &entries, profile, reader.budget, cancelled)? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let middle = entries.len() / 2;
    for half in [&entries[..middle], &entries[middle..]] {
        if !repack::borrowed_fits_fresh::<_, N>(half, profile, reader.budget, cancelled)? {
            return tree::continue_index::<_, N>(operation, append, reader.budget, cancelled);
        }
    }
    // Every role possibly changed by DATA plus the selected physical index is
    // selected explicitly. Canonical aliases remain one physical owner.
    let definition = schema.definition_page();
    append.require_source_map_exclusive(
        image,
        &[
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition },
            Owner::OrdinaryCandidate { definition },
            Owner::IndexOwned {
                definition,
                physical: index.data_number(),
            },
        ],
        &mut reader,
    )?;
    prior_allocations(&append, &mut reader)?;

    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    let mut patches = Vec::new();
    edit_prepared(
        &nonroot::Operation::<_, N> {
            image,
            index,
            profile,
            keys: operation.final_keys,
        },
        &tree,
        index
            .columns()
            .first()
            .ok_or(InspectError::Unsupported)?
            .number(),
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

fn physical_index<'a, P: RetainedProfile>(
    operation: &Operation<'a, P>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    let index = super::super::insertion::qualify(
        operation.checked,
        operation.index.data_number(),
        budget,
        cancelled,
    )?;
    if index.root_page() != operation.index.root_page()
        || index.usage_map() != operation.index.usage_map()
        || index.flags() != operation.index.flags()
        || index.columns() != operation.index.columns()
    {
        return Err(EditError::Preservation);
    }
    Ok(index)
}

fn retained_members<P: RetainedProfile, const N: usize>(
    original: &[KeyRef<P::Key>],
    final_keys: &[KeyRef<P::Key>],
    profile: &P,
    index: &NativeIndex,
    append: &State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if final_keys.len() <= original.len() {
        return Err(EditError::Conflict);
    }
    let mut position = 0;
    membership::require_constraints(index, final_keys, profile, reader)?;
    let mut previous: Option<&KeyRef<P::Key>> = None;
    for (number, entry) in final_keys.iter().enumerate() {
        reader.tick()?;
        // Re-encoding validates width and NULL marker for the bound profile.
        super::super::encoding::borrowed_entry::<_, N>(
            &crate::table::indexes::page::Entry {
                value: KeyRef {
                    key: &entry.key,
                    row: entry.row,
                },
                child: 0,
            },
            profile,
        )?;
        if previous.is_some_and(|prior| !prior.compare_with(entry, profile).is_lt()) {
            return Err(EditError::Conflict);
        }
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
        if original
            .get(position)
            .is_some_and(|old| old.compare_with(entry, profile).is_eq())
        {
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

fn require_prior_owners<P: RetainedProfile>(
    operation: &Operation<'_, P>,
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
        if prior.physical >= operation.index.data_number() || !matched {
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
        match global.future_cell(reader, number) {
            Ok(cell) => {
                if cell.before & cell.mask == 0 {
                    return Err(EditError::Preservation);
                }
            }
            Err(InspectError::Unsupported) if append.global_record().is_some() => {}
            Err(error) => return Err(error.into()),
        }
        if append.global_is_free(&global, number, reader)? {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}
