//! Recursive original-bound INSERT layout with one retained-root growth event.
mod layout;
mod source;
mod write;

use super::super::{AllocationPolicy, Plan, allocation, leaf_entries, pages, seal};
use super::{
    ENTRY_BUFFER, Operation, physical_index, prior_allocations, require_prior_owners,
    retained_members,
};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::{
        indexes::{
            header::IndexHeader,
            mutation::{IndexDataPatch, NumericKey, profile::SingleProfile},
            page::{LEAF, NODE},
        },
        mutation::{
            append::State,
            map_authority::{self, Owner},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

/// Bind original topology and immutable final membership, preflight all levels,
/// then resolve the same DATA cursor into final INDEX/bitmap reservations.
pub(super) fn continue_primary(
    operation: Operation<'_>,
    append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let image = operation.image;
    let checked = operation.checked;
    let definition = checked.schema().definition_page();
    let index = physical_index(&operation, budget, cancelled)?;
    if append.original_length != image.as_bytes().len()
        || append.owner != definition
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
        column: checked.schema().columns()[operation.id].number(),
        kind: NumericKey::new(
            checked.schema().columns()[operation.id]
                .scalar_type()
                .ok_or(InspectError::Unsupported)?,
        )?,
        descending: index.columns()[0].descending(),
    };
    let header = IndexHeader::read(raw(image, index.root_page())?)?;
    if !matches!(header.kind, NODE | LEAF) || header.previous.get() != 0 || header.next.get() != 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let source =
        source::capture::<_, ENTRY_BUFFER>(image, index, definition, &profile, &mut reader)?;
    super::super::super::insertion::require_population(
        index,
        &source.keys,
        checked.rows(),
        &mut reader,
    )?;
    if operation.final_keys.len() == source.keys.len() {
        reader.work(source.keys.len())?;
        if operation.final_keys != source.keys {
            return Err(EditError::Conflict);
        }
    } else {
        retained_members(
            &source.keys,
            operation.final_keys,
            &profile,
            index,
            &append,
            &mut reader,
        )?;
    }
    let layout = if operation.final_keys.is_empty() {
        None
    } else {
        Some(layout::prepare::<_, ENTRY_BUFFER>(
            image,
            &source,
            operation.final_keys,
            &profile,
            &mut reader,
        )?)
    };
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
    let mut owned = crate::format::allocation::pages(&mut reader, index.usage_map())?;
    let mut expected_owned = Vec::new();
    source::reserve(&mut expected_owned, source.nodes.len(), &mut reader)?;
    for node in &source.nodes {
        reader.tick()?;
        expected_owned.push(node.number);
    }
    if owned.len() != expected_owned.len() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let sort_work = expected_owned
        .len()
        .checked_mul(expected_owned.len())
        .ok_or(InspectError::StructureLimit)?;
    reader.work(sort_work)?;
    owned.sort_unstable();
    expected_owned.sort_unstable();
    if owned != expected_owned {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    prior_allocations(&append, &mut reader)?;
    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    let Some(layout) = layout else {
        check(reader.budget, cancelled)?;
        return Ok(plan);
    };
    let mut numbers = Vec::new();
    source::reserve(&mut numbers, layout.nodes.len(), &mut reader)?;
    for node in &layout.nodes {
        reader.tick()?;
        numbers.push(match node.origin {
            layout::Origin::Existing(number) | layout::Origin::GrownRoot(number) => number,
            layout::Origin::Fresh => 0,
        });
    }
    for &fresh in &layout.fresh {
        reader.tick()?;
        numbers[fresh] = allocation::reserve(image, index, &mut plan, &mut reader)?;
    }
    write::emit::<_, ENTRY_BUFFER>(
        write::Operation {
            image,
            keys: operation.final_keys,
            profile: &profile,
            layout: &layout,
            numbers: &numbers,
        },
        &mut plan,
        &mut reader,
    )?;
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

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let start = (number as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    image
        .as_bytes()
        .get(start..end)
        .ok_or(InspectError::Truncated.into())
}

fn patch(
    number: u32,
    before: &[u8],
    after: Vec<u8>,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(PAGE_BYTES)?;
    if before == after {
        return Ok(());
    }
    reader
        .budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut original = Vec::new();
    original
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    original.extend_from_slice(before);
    reader
        .budget
        .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    plan.patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.patches.push(IndexDataPatch {
        offset: number as usize * PAGE_BYTES,
        before: original,
        after,
    });
    Ok(())
}

#[cfg(test)]
mod tests;
