//! Fresh source, complete membership and exact-byte complement certification.
mod definition;
mod framing;

use super::{allocation_error, source};
use crate::{
    EditError, IndexCheckStatus, IndexProjectionSelection, IndexReadRequest, InspectError,
    NativeImage,
    header::check,
    inspect_table_index_counter, read_table_index_with_references,
    table::{
        indexes::check_table_retained,
        mutation::{
            index_creation::{SecondaryIndexCreate, buffer},
            normal_ordinary,
        },
        table_append::Inventory,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const PAGE: usize = 4096;

pub(super) fn prove(
    before: &NativeImage,
    after: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let original = source::capture(before, table, request, budget, cancelled)?;
    prove_original(before, after, table, request, original, budget, cancelled)
}

pub(super) fn prove_fresh(
    before: &NativeImage,
    after: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let original = source::capture_fresh(before, table, request, budget, cancelled)?;
    prove_original(before, after, table, request, original, budget, cancelled)
}

fn prove_original(
    before: &NativeImage,
    after: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    original: source::Original,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let keys = original.keys(budget, cancelled)?;
    let inventory = Inventory::capture(before, budget, cancelled)?;
    let after_pages =
        u32::try_from(after.as_bytes().len() / PAGE).map_err(|_| InspectError::StructureLimit)?;
    let allocation = inventory
        .plan_allocation(after_pages, budget, cancelled)
        .map_err(allocation_error)?;
    allocation
        .verify_after(after.as_bytes(), budget, cancelled)
        .map_err(allocation_error)?;
    Inventory::capture(after, budget, cancelled)?.require_source(after)?;
    let observed = normal_ordinary::capture_retained_schema(after, table, budget, cancelled)?;
    let indexes = observed.indexes.indexes();
    if indexes.len()
        != original
            .content
            .indexes
            .indexes()
            .len()
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?
    {
        return Err(EditError::Preservation);
    }
    let index = indexes.last().ok_or(EditError::Preservation)?;
    let [column] = index.columns() else {
        return Err(EditError::Preservation);
    };
    if index.name() != original.name.as_str()
        || index.number() != original.logical
        || index.data_number() != original.layout.physical_count
        || index.flags() != 128
        || index.native_type() != 0
        || column.number() != original.native
        || column.descending() != request.descending
        || observed.records.len() != keys.len()
        || !observed.dependencies.is_empty()
    {
        return Err(EditError::Preservation);
    }
    let host = index.usage_map() / 256;
    if index.usage_map() % 256 != 0
        || host <= original.index_start
        || host >= after_pages
        || index.root_page() < original.index_start
        || index.root_page() >= host
    {
        return Err(EditError::Preservation);
    }
    let scope = before.catalog().catalog().scope();
    let checked = check_table_retained(&mut after.reader(), scope, table, budget, cancelled)?;
    if checked.rows() != keys.len()
        || checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Preservation);
    }
    let names = [request.column];
    let indexed = read_table_index_with_references(
        &mut after.reader(),
        scope,
        table,
        original.name.as_str(),
        IndexReadRequest {
            columns: Some(&names),
            selection: IndexProjectionSelection::All,
            maximum_rows: u32::try_from(keys.len().max(1))
                .map_err(|_| InspectError::StructureLimit)?,
        },
        budget,
        cancelled,
    )?;
    if indexed.snapshot().rows().len() != keys.len() || indexed.row_references().len() != keys.len()
    {
        return Err(EditError::Preservation);
    }
    let mut seen = buffer::vector(keys.len(), budget, cancelled)?;
    seen.resize(keys.len(), false);
    let mut previous = None;
    let mut distinct = 0_u32;
    for (row, reference) in indexed
        .snapshot()
        .rows()
        .iter()
        .zip(indexed.row_references())
    {
        check(budget, cancelled)?;
        let reference = reference
            .page_number()
            .checked_mul(256)
            .and_then(|page| page.checked_add(u32::from(reference.slot_number())))
            .ok_or(InspectError::StructureLimit)?;
        // Binary search is paid at each comparison, without an uninterruptible
        // sorting pass or a producer topology/key-vector dependency.
        let mut low = 0;
        let mut high = keys.len();
        while low < high {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let middle = low + (high - low) / 2;
            if keys[middle].reference < reference {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let expected = keys
            .get(low)
            .filter(|key| key.reference == reference)
            .ok_or(EditError::Preservation)?;
        let value = match row.as_slice() {
            [Value::Null] => None,
            [Value::Int32(value)] => Some(i64::from(*value)),
            _ => return Err(EditError::Preservation),
        };
        if seen[low] || expected.value != value {
            return Err(EditError::Preservation);
        }
        seen[low] = true;
        if let Some((old, old_reference)) = previous {
            let order = if request.descending {
                value.cmp(&old).reverse()
            } else {
                value.cmp(&old)
            };
            if order.is_lt() || (order.is_eq() && reference <= old_reference) {
                return Err(EditError::Preservation);
            }
        }
        if previous.is_none_or(|(old, _)| old != value) {
            distinct = distinct
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
        }
        previous = Some((value, reference));
    }
    for value in seen {
        check(budget, cancelled)?;
        if !value {
            return Err(EditError::Preservation);
        }
    }
    let counter = inspect_table_index_counter(
        &mut after.reader(),
        scope,
        table,
        original.name.as_str(),
        budget,
        cancelled,
    )?;
    if counter.value() != distinct {
        return Err(EditError::Preservation);
    }
    framing::appended(
        after.as_bytes(),
        framing::Appended {
            first: original.index_start,
            host,
            root: index.root_page(),
            owner: original.layout.definition,
            descending: request.descending,
        },
        budget,
        cancelled,
    )?;
    let definition = definition::definition(
        before,
        &original,
        index.root_page(),
        host,
        distinct,
        budget,
        cancelled,
    )?;
    let definition_start = original.layout.definition as usize * PAGE;
    if after
        .as_bytes()
        .get(definition_start..definition_start + PAGE)
        != Some(definition.as_slice())
    {
        return Err(EditError::Preservation);
    }
    // Exact old-image complement, excluding only the independently reconstructed
    // definition and allocator's sealed byte ranges (already verified above).
    for (offset, (left, right)) in before
        .as_bytes()
        .chunks(PAGE)
        .zip(after.as_bytes().chunks(PAGE))
        .enumerate()
    {
        check(budget, cancelled)?;
        let base = offset * PAGE;
        if base == definition_start {
            continue;
        }
        for (within, (left, right)) in left.iter().zip(right).enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            if left != right
                && !allocation
                    .allowed_before_ranges()
                    .iter()
                    .any(|range| range.contains(&(base + within)))
            {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}
