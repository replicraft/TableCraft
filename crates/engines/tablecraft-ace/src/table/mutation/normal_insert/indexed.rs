//! Original-derived fitting manual INSERT, grouped by physical index owner.
pub(super) mod proof;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        data_type::{CURRENCY, INT64},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_index_counter,
    limits::MAX_LIVE_ROWS,
    table::{
        indexes::mutation::{
            insertion::{self, InsertRow},
            normal_index_audit::Ownership,
        },
        mutation::{
            fixed_records::{self, indexed::CheckedIndexes},
            lval_ownership::Inventory,
            preserved_source::Source,
            record_insertion::properties,
        },
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LEAF_KIND: u8 = 4;
const NODE_KIND: u8 = 3;
const PAGE_VERSION: u8 = 1;
const OWNER_AT: usize = 4;
const PREVIOUS_AT: usize = 12;
const NEXT_AT: usize = 16;
const PREFIX_AT: usize = 24;
const TAIL_AT: usize = 20;
const LOCAL_INDEX_KIND_MAX: u8 = 1;
const STAMP_START: usize = 8;
const STAMP_END: usize = 12;
const LEVEL_AT: usize = 26;

struct History {
    offset: usize,
    before: u32,
    after: u32,
}

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.ordinary.insert.indexed").entered();
    let (facts, _) = qualify(&image, &source, table, requests, budget, cancelled)?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::records::qualify(source.table.schema(), requests, budget, cancelled)?;

    source
        .records
        .len()
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    let records = super::records::encode(source.table.schema(), requests, budget, cancelled)?;
    let placement = super::placement::reserve(&image, &source, &records, budget, cancelled)?;

    let mut rows = vector(requests.len(), budget, cancelled)?;
    for (reference, request) in placement.references().zip(requests) {
        check(budget, cancelled)?;
        rows.push(InsertRow {
            reference,
            values: request.values(),
        });
    }

    let count = facts.checked().schema().layout.physical_indexes;
    let mut indexes = vector(count, budget, cancelled)?;
    let mut history = vector(count, budget, cancelled)?;
    for physical in 0..count {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let index = insertion::prepare_existing(
            &image,
            facts.checked(),
            physical,
            &rows,
            budget,
            cancelled,
        )?;
        if index.physical() != physical
            || index.original_count() != source.records.len()
            || index.expected().len() != source.records.len() + requests.len()
        {
            return Err(EditError::Preservation);
        }
        let selected = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        if index.root() != selected.root_page()
            || index.usage() != selected.usage_map()
            || index.source_pages().first() != Some(&selected.root_page())
        {
            return Err(EditError::Preservation);
        }
        let original = inspect_table_index_counter(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            selected.name(),
            budget,
            cancelled,
        )?;
        let delta =
            u32::try_from(index.distinct_added()).map_err(|_| InspectError::StructureLimit)?;
        if original.definition_page() != source.table.schema().definition_page()
            || original.physical_index() != physical
            || index.distinct_added() > requests.len()
        {
            return Err(EditError::Preservation);
        }
        let next = original
            .value()
            .checked_add(delta)
            .ok_or(InspectError::StructureLimit)?;
        history.push(History {
            offset: original.byte_range().start,
            before: original.value(),
            after: next,
        });
        indexes.push(index);
    }

    // All rows, indexes, final uniqueness and historical arithmetic are checked
    // before the first candidate bytes exist. Existing NODE partitions may
    // be repacked; splits and allocation remain unavailable.
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    placement.apply(&image, &records, &mut after, budget, cancelled)?;
    for index in &indexes {
        for patch in &index.patches {
            apply(
                &mut after,
                patch.offset,
                &patch.before,
                &patch.after,
                budget,
                cancelled,
            )?;
        }
    }
    for change in &history {
        apply(
            &mut after,
            change.offset,
            &change.before.to_le_bytes(),
            &change.after.to_le_bytes(),
            budget,
            cancelled,
        )?;
    }

    drop(facts);
    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: requests.len(),
    };
    proof::prove(&plan, scope, table, requests, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn qualify<'image>(
    image: &'image NativeImage,
    source: &Source,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(CheckedIndexes<'image>, bool), EditError> {
    let facts = CheckedIndexes::capture_actions(image, table, budget, cancelled)?;
    let schema = source.table.schema();
    facts.require_source(image, schema, budget, cancelled)?;
    if facts.checked().schema().declared_rows() != schema.declared_rows()
        || facts.checked().rows() != source.records.len()
    {
        return Err(EditError::Preservation);
    }
    let ownership = Ownership::capture(image, facts.checked(), budget, cancelled)?;
    let live = Inventory::capture(image, budget, cancelled)?;
    // A proof-wire choice derived from already-paid native checks, never caller
    // authority. Keep the existing ASC/present reconstruction and its resource
    // and cancellation trace unchanged for that established family.
    let mut optional_wire = false;
    for physical in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let index = insertion::qualify_actions(facts.checked(), physical, budget, cancelled)?;
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        optional_wire |= key.descending();
        if index.native_type() > LOCAL_INDEX_KIND_MAX
            || index
                .constraints()
                .ok_or(InspectError::Unsupported)?
                .ignore_nulls()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let tree = ownership.require(image, schema, index, budget, cancelled)?;
        live.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
        let base = index.root_page() as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let leaf = raw[0] == LEAF_KIND;
        if (!leaf && raw[0] != NODE_KIND)
            || raw[1] != PAGE_VERSION
            || raw[STAMP_START..STAMP_END] != [0; STAMP_END - STAMP_START]
            || (leaf && raw[LEVEL_AT] != 0)
            || u32_at(raw, OWNER_AT)? != schema.definition_page()
            || u32_at(raw, PREVIOUS_AT)? != 0
            || u32_at(raw, NEXT_AT)? != 0
            || (leaf
                && (tree.pages() != [index.root_page()]
                    || u16_at(raw, PREFIX_AT)? != 0
                    || u32_at(raw, TAIL_AT)? != 0))
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }

        let mut column = None;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if field.number() == key.number() {
                column = Some(position);
            }
        }
        let column = column.ok_or(EditError::Preservation)?;
        let field = &schema.columns()[column];
        let integral = matches!(field.native_type(), INT64 | CURRENCY);
        if integral && (field.number() != column || !leaf) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        for row in source.table.rows() {
            check(budget, cancelled)?;
            match (field.native_type(), row.get(column)) {
                (INT64, Some(Value::Int64(_))) | (CURRENCY, Some(Value::Currency(_))) => {}
                (_, Some(Value::Int32(_))) if !integral => {}
                (_, Some(Value::Null)) => optional_wire = true,
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            }
        }
        for request in requests {
            check(budget, cancelled)?;
            match (field.native_type(), request.values().get(column)) {
                (INT64, Some(Value::Int64(_))) | (CURRENCY, Some(Value::Currency(_))) => {}
                (_, Some(Value::Int32(_))) if !integral => {}
                (_, Some(Value::Null)) => optional_wire = true,
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            }
        }
    }
    check(budget, cancelled)?;
    Ok((facts, optional_wire))
}

fn apply(
    after: &mut [u8],
    offset: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let end = offset
        .checked_add(before.len())
        .ok_or(InspectError::StructureLimit)?;
    let destination = after.get_mut(offset..end).ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, (before.len() + replacement.len()) as u64)
        .map_err(InspectError::from)?;
    if before.len() != replacement.len() || destination != before {
        return Err(EditError::Preservation);
    }
    destination.copy_from_slice(replacement);
    Ok(())
}

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}
