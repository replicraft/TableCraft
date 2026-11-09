//! Independent original membership, record placement and raw root promotion.
mod data;

use crate::{
    EditError, IndexConstraintViolation, InspectError, NativeColumn, NativeEditPlan, NativeImage,
    RowInsert, TableSchema,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u16_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    inspect_table_index_counter,
    table::{
        indexes::{header::IndexHeader, page::ENTRY_START},
        record_layout,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const MASK_AT: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_AT;
const FREE_AT: usize = std::mem::offset_of!(IndexHeader, free);
const OWNER_AT: usize = std::mem::offset_of!(IndexHeader, owner);
const PREVIOUS_AT: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_AT: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_AT: usize = std::mem::offset_of!(IndexHeader, tail);
const PREFIX_AT: usize = std::mem::offset_of!(IndexHeader, prefix);
const LEVEL_AT: usize = PREFIX_AT + size_of::<u16>();
const MARKER: u8 = 0x7f;
const SIGN: u32 = 1 << (u32::BITS - 1);
const KEY_BYTES: usize = size_of::<u8>() + size_of::<i32>();
const WIRE_BYTES: usize = KEY_BYTES + size_of::<u32>();
const NODE_BYTES: usize = WIRE_BYTES + size_of::<u32>();
const LEAF: u8 = 4;
const NODE: u8 = 3;
const VERSION: u8 = 1;

type Key = (i32, u32);

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.insert.root_split.prove").entered();
    let compound =
        plan.before_bytes().len().checked_add(3 * PAGE_BYTES) == Some(plan.after_bytes().len());
    if (!compound
        && plan.before_bytes().len().checked_add(2 * PAGE_BYTES) != Some(plan.after_bytes().len()))
        || plan.changed_rows() != requests.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture_action_source(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    if !super::selected(&image, &source, requests)? {
        return Err(EditError::Preservation);
    }
    let (facts, column) =
        super::admission::qualify(&image, &source, table, requests, budget, cancelled)?;
    let index =
        crate::table::indexes::mutation::insertion::qualify(facts.checked(), 0, budget, cancelled)?;
    let authority = if compound {
        super::authority::Authority::capture_after_data(
            &image,
            source.table.schema(),
            index,
            budget,
            cancelled,
        )?
    } else {
        super::authority::Authority::capture(
            &image,
            source.table.schema(),
            index,
            budget,
            cancelled,
        )?
    };
    authority.require(&image, source.table.schema(), index)?;
    crate::table::mutation::record_insertion::properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::super::records::qualify(source.table.schema(), requests, budget, cancelled)?;
    let (mut expected, references) = if compound {
        data::reconstruct(&image, &source, requests, budget, cancelled)?
    } else {
        super::super::proof::reconstruct_with_references(
            &image, &source, requests, budget, cancelled,
        )?
    };
    if references.len() != requests.len() || source.table.rows().len() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let capacity = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut keys = vector(capacity, budget)?;
    let field = &source.table.schema().columns()[column];
    for (record, row) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = original_value(
            &record.data,
            source.table.schema(),
            field,
            row.get(column).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        keys.push((value, record.reference));
    }
    sort(&mut keys, budget, cancelled)?;
    let root = index.root_page() as usize * PAGE_BYTES;
    let original = image
        .as_bytes()
        .get(root..root + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    original_membership(original, &keys, budget, cancelled)?;
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let Some(Value::Int32(value)) = request.values().get(column) else {
            return Err(EditError::Preservation);
        };
        keys.push((*value, reference));
    }
    sort(&mut keys, budget, cancelled)?;
    for pair in keys.windows(2) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<Key>() as u64)
            .map_err(InspectError::from)?;
        if pair[0].0 == pair[1].0 || pair[0].1 == pair[1].1 {
            return Err(EditError::Preservation);
        }
    }
    let middle = keys.len() / 2;
    if middle == 0
        || middle * WIRE_BYTES > PAGE_BYTES - ENTRY_START
        || (keys.len() - middle) * WIRE_BYTES > PAGE_BYTES - ENTRY_START
    {
        return Err(EditError::Preservation);
    }
    let right = image
        .catalog()
        .header()
        .page_count()
        .checked_add(u32::from(compound))
        .ok_or(InspectError::StructureLimit)?;
    let left = right.checked_add(1).ok_or(InspectError::StructureLimit)?;
    let definition = source.table.schema().definition_page();
    let right_page = leaf(definition, left, 0, &keys[middle..], budget, cancelled)?;
    let left_page = leaf(definition, 0, right, &keys[..middle], budget, cancelled)?;
    budget
        .charge_allocation((2 * PAGE_BYTES) as u64, 1)
        .map_err(InspectError::from)?;
    expected
        .try_reserve_exact(2 * PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, (2 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    expected.extend_from_slice(&right_page);
    expected.extend_from_slice(&left_page);

    let root_page = &mut expected[root..root + PAGE_BYTES];
    root_page[0] = NODE;
    root_page[LEVEL_AT] = 1;
    root_page[TAIL_AT..TAIL_AT + size_of::<u32>()].copy_from_slice(&right.to_le_bytes());
    root_page[PREFIX_AT..PREFIX_AT + size_of::<u16>()].fill(0);
    root_page[FREE_AT..FREE_AT + size_of::<u16>()].copy_from_slice(
        &u16::try_from(PAGE_BYTES - ENTRY_START - NODE_BYTES)
            .map_err(|_| EditError::Preservation)?
            .to_le_bytes(),
    );
    root_page[MASK_AT..ENTRY_START].fill(0);
    root_page[MASK_AT + NODE_BYTES / MASK_BITS] = 1 << (NODE_BYTES % MASK_BITS);
    root_page[ENTRY_START..ENTRY_START + WIRE_BYTES].copy_from_slice(&wire(keys[middle - 1]));
    root_page[ENTRY_START + WIRE_BYTES..ENTRY_START + NODE_BYTES]
        .copy_from_slice(&left.to_be_bytes());
    budget
        .charge(
            Resource::Steps,
            (MASK_BYTES + NODE_BYTES + 2 * size_of::<u32>() + 2 * size_of::<u16>() + 2) as u64,
        )
        .map_err(InspectError::from)?;
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        scope,
        table,
        index.name(),
        budget,
        cancelled,
    )?;
    let history = counter
        .value()
        .checked_add(u32::try_from(requests.len()).map_err(|_| InspectError::StructureLimit)?)
        .ok_or(InspectError::StructureLimit)?;
    expected[counter.byte_range()].copy_from_slice(&history.to_le_bytes());
    maps(
        &image,
        index.usage_map(),
        compound,
        &mut expected,
        budget,
        cancelled,
    )?;
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture_action_source(&after, table, budget, cancelled)?;
    if final_source.table.schema().object_id() != source.table.schema().object_id()
        || final_source.table.schema().definition_page() != definition
        || final_source.records.len() != capacity
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn original_value(
    raw: &[u8],
    schema: &TableSchema,
    field: &NativeColumn,
    observed: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<i32, EditError> {
    check(budget, cancelled)?;
    let layout = record_layout::parse(raw, schema, budget)?;
    if field.number() >= layout.count {
        return Err(EditError::Preservation);
    }
    let mask = layout
        .mask_start
        .checked_add(field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    if raw.get(mask).ok_or(EditError::Preservation)? & (1 << (field.number() % MASK_BITS)) == 0 {
        return Err(InspectError::IndexConstraint(IndexConstraintViolation::NullKey).into());
    }
    let start = COLUMN_COUNT_BYTES
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    if end > layout.mask_start {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    let value = i32::from_le_bytes(
        raw.get(start..end)
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    );
    if observed != &Value::Int32(value) {
        return Err(EditError::Preservation);
    }
    Ok(value)
}

fn wire((value, reference): Key) -> [u8; WIRE_BYTES] {
    let mut result = [0; WIRE_BYTES];
    result[0] = MARKER;
    result[1..KEY_BYTES].copy_from_slice(&((value as u32) ^ SIGN).to_be_bytes());
    result[KEY_BYTES..].copy_from_slice(&reference.to_be_bytes());
    result
}

fn original_membership(
    page: &[u8],
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let seed = usize::from(u16_at(page, PREFIX_AT)?);
    if seed >= WIRE_BYTES {
        return Err(EditError::Preservation);
    }
    let mut bitmap = [0; MASK_BYTES];
    let mut active = 0_usize;
    let first = keys.first().copied().map(wire);
    for (position, key) in keys.iter().enumerate() {
        check(budget, cancelled)?;
        let encoded = wire(*key);
        if position != 0
            && first.as_ref().ok_or(EditError::Preservation)?[..seed] != encoded[..seed]
        {
            return Err(EditError::Preservation);
        }
        let stored = if position == 0 {
            &encoded[..]
        } else {
            &encoded[seed..]
        };
        budget
            .charge(Resource::Steps, stored.len() as u64)
            .map_err(InspectError::from)?;
        let end = active
            .checked_add(stored.len())
            .ok_or(InspectError::StructureLimit)?;
        if end > PAGE_BYTES - ENTRY_START {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, (2 * WIRE_BYTES + seed) as u64)
            .map_err(InspectError::from)?;
        if page.get(ENTRY_START + active..ENTRY_START + end) != Some(stored) {
            return Err(EditError::Preservation);
        }
        active = end;
        bitmap[end / MASK_BITS] |= 1 << (end % MASK_BITS);
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if page[MASK_AT..ENTRY_START] != bitmap
        || usize::from(u16_at(page, FREE_AT)?) != PAGE_BYTES - ENTRY_START - active
    {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn leaf(
    definition: u32,
    previous: u32,
    next: u32,
    keys: &[Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let mut page = vector(PAGE_BYTES, budget)?;
    page.resize(PAGE_BYTES, 0);
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    page[..2].copy_from_slice(&[LEAF, VERSION]);
    page[OWNER_AT..OWNER_AT + size_of::<u32>()].copy_from_slice(&definition.to_le_bytes());
    page[PREVIOUS_AT..PREVIOUS_AT + size_of::<u32>()].copy_from_slice(&previous.to_le_bytes());
    page[NEXT_AT..NEXT_AT + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
    let active = keys
        .len()
        .checked_mul(WIRE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    if active > PAGE_BYTES - ENTRY_START {
        return Err(EditError::Preservation);
    }
    page[FREE_AT..FREE_AT + size_of::<u16>()].copy_from_slice(
        &u16::try_from(PAGE_BYTES - ENTRY_START - active)
            .map_err(|_| EditError::Preservation)?
            .to_le_bytes(),
    );
    for (position, key) in keys.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * WIRE_BYTES) as u64)
            .map_err(InspectError::from)?;
        let end = (position + 1) * WIRE_BYTES;
        page[ENTRY_START + end - WIRE_BYTES..ENTRY_START + end].copy_from_slice(&wire(*key));
        page[MASK_AT + end / MASK_BITS] |= 1 << (end % MASK_BITS);
    }
    Ok(page)
}

fn maps(
    image: &NativeImage,
    usage: u32,
    compound: bool,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let owned = RepresentedMap::read(&mut reader, usage, MapRole::Owned)?;
    let first = image
        .catalog()
        .header()
        .page_count()
        .checked_add(u32::from(compound))
        .ok_or(InspectError::StructureLimit)?;
    for number in [
        first,
        first.checked_add(1).ok_or(InspectError::StructureLimit)?,
    ] {
        for (set, map) in [(false, &global), (true, &owned)] {
            reader.tick()?;
            let cell = map.future_cell(&mut reader, number)?;
            if (cell.before & cell.mask != 0) == set {
                return Err(EditError::Preservation);
            }
            if set {
                expected[cell.offset] |= cell.mask;
            } else {
                expected[cell.offset] &= !cell.mask;
            }
        }
    }
    Ok(())
}

fn sort(
    keys: &mut [Key],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(size_of::<Key>()))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(())
}

fn vector<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}
