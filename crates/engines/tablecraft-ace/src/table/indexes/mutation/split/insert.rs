//! Cardinality-growing primary continuation; DATA authority stays with INSERT.
#[cfg(test)]
mod tests;

use super::super::{NumericKey, profile::SingleProfile, repack, tree::Tree};
use super::{AllocationPolicy, Plan, edit_prepared, leaf_entries, nonroot, seal};
use crate::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, NativeImage, NativeIndex,
    TableCheck, WriteBlocker,
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
            page::{KeyRef, LEAF},
        },
        mutation::{
            append::{self, State},
            map_authority::{self, Owner},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const PRIMARY_KIND: u8 = 1;
const PHYSICAL_COUNT: usize = 1;
const PHYSICAL_NUMBER: u32 = 0;
const ENTRY_BUFFER: usize = size_of::<u8>() + size_of::<i32>() + 2 * size_of::<u32>();

/// Final sorted keys must come from immutable INSERT requests and certified DATA
/// placements. This constructor does not certify new records or publish bytes.
pub(in crate::table::indexes) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub checked: &'a TableCheck,
    pub id: usize,
    pub final_keys: &'a [KeyRef],
}

/// Continue only an overflowing original primary leaf whose two halves fit.
/// Existing represented/fitted INSERT remains a separate caller path. This does
/// not invoke UPDATE key preparation or the unchanged-cardinality arena.
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
    let index = primary(&operation, budget, cancelled)?;
    if append.original_length != image.as_bytes().len()
        || append.owner != schema.definition_page()
        || append.length < append.original_length
        || append.length as u64 > MAX_IMAGE_BYTES
        || !append.length.is_multiple_of(PAGE_BYTES)
        || (append.length - append.original_length) / PAGE_BYTES != append.reservations_len()
        || append.indexes().next().is_some()
    {
        return Err(EditError::Preservation);
    }
    append.preview(0)?;
    if operation.final_keys.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let profile = SingleProfile {
        column: schema.columns()[operation.id].number(),
        kind: NumericKey::new(ScalarType::Int32)?,
        descending: false,
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
    retained_members(&original_keys, operation.final_keys, &append, &mut reader)?;
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
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    // Every role possibly changed by DATA plus the sole physical primary is
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

fn primary<'a>(
    operation: &Operation<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeIndex, EditError> {
    let checked = operation.checked;
    let schema = checked.schema();
    let field = schema
        .columns()
        .get(operation.id)
        .filter(|field| field.scalar_type() == Some(ScalarType::Int32))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if schema.layout.physical_indexes != PHYSICAL_COUNT || checked.indexes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let first = checked.indexes()[0].index();
    let mut primary = None;
    for checked_index in checked.indexes() {
        check(budget, cancelled)?;
        let index = checked_index.index();
        let constraints = index
            .constraints()
            .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
        if checked_index.status() != IndexCheckStatus::Checked
            || index.native_type() > PRIMARY_KIND
            || index.data_number() != PHYSICAL_NUMBER
            || index.root_page() != first.root_page()
            || index.usage_map() != first.usage_map()
            || index.flags() != first.flags()
            || index.columns() != first.columns()
            || index.columns().len() != 1
            || index.columns()[0].number() != field.number()
            || index.columns()[0].descending()
            || !constraints.unique()
            || !constraints.required()
            || constraints.ignore_nulls()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if constraints.primary_key() && primary.replace(index).is_some() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    primary.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))
}

fn retained_members(
    original: &[KeyRef],
    final_keys: &[KeyRef],
    append: &State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if final_keys.len() <= original.len() {
        return Err(EditError::Conflict);
    }
    let mut position = 0;
    let mut previous = None;
    for (number, entry) in final_keys.iter().enumerate() {
        reader.tick()?;
        let value = entry
            .key
            .filter(|value| i32::try_from(*value).is_ok())
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if previous == Some(value) {
            return Err(
                InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey).into(),
            );
        }
        if previous.is_some_and(|prior| prior > value) {
            return Err(EditError::Conflict);
        }
        previous = Some(value);
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
            .is_some_and(|old| old.key <= entry.key)
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
