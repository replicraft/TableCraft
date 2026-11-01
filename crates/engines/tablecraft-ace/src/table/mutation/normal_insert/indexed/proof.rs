//! Fresh original/request replay; no insertion patches or prepared keys enter.
mod optional;
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, NativeIndex, RowInsert,
    format::{
        bytes::{u16_at, u32_at},
        page::PAGE_BYTES,
    },
    header::check,
    inspect_table_index_counter,
    table::mutation::preserved_source::Source,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const FREE_AT: usize = 2;
const KEY_AT: usize = 480;
const MASK_AT: usize = 27;
const MASK_BYTES: usize = 453;
const WIRE_BYTES: usize = 9;
const PRESENT_MARKER: u8 = 0x7f;
const SIGN_FLIP: u32 = 0x8000_0000;

struct RequestedRows<'a> {
    requests: &'a [RowInsert],
    references: &'a [u32],
}

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.insert.indexed.prove").entered();
    if plan.before_bytes().len() != plan.after_bytes().len()
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
    let source = super::super::capture_source(&image, table, budget, cancelled)?;
    let (facts, optional_wire) =
        super::qualify(&image, &source, table, requests, budget, cancelled)?;
    crate::table::mutation::record_insertion::properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::super::records::qualify(source.table.schema(), requests, budget, cancelled)?;

    // This is the separate original-capacity DATA reconstruction used by the
    // original unindexed proof. It accepts no staged selections or bytes.
    let (mut expected, references) = super::super::proof::reconstruct_with_references(
        &image, &source, requests, budget, cancelled,
    )?;
    let schema = source.table.schema();
    for physical in 0..schema.layout.physical_indexes {
        check(budget, cancelled)?;
        let physical = u32::try_from(physical).map_err(|_| InspectError::StructureLimit)?;
        let index = crate::table::indexes::mutation::insertion::qualify(
            facts.checked(),
            physical,
            budget,
            cancelled,
        )?;
        let rows = RequestedRows {
            requests,
            references: &references,
        };
        if optional_wire {
            optional::reconstruct(
                &image,
                &source,
                index,
                rows,
                &mut expected,
                budget,
                cancelled,
            )?;
        } else {
            reconstruct_index(
                &image,
                &source,
                index,
                rows,
                &mut expected,
                budget,
                cancelled,
            )?;
        }
    }
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    // Recapture complete final fields/properties/constraints and every physical
    // alias from the independently reconstructed full image, without effects.
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture_source(&after, table, budget, cancelled)?;
    let (final_facts, _) = super::qualify(&after, &final_source, table, &[], budget, cancelled)?;
    if final_source.records.len() != source.records.len() + requests.len()
        || final_source.table.schema().object_id() != schema.object_id()
        || final_source.table.schema().definition_page() != schema.definition_page()
        || final_facts.checked().indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn reconstruct_index(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    rows: RequestedRows<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let RequestedRows {
        requests,
        references,
    } = rows;
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
    let mut column = None;
    for (position, field) in source.table.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == key.number() {
            column = Some(position);
        }
    }
    let column = column.ok_or(EditError::Preservation)?;
    let capacity = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut original = super::vector(source.records.len(), budget, cancelled)?;
    for (row, record) in source.table.rows().iter().zip(&source.records) {
        check(budget, cancelled)?;
        let Some(Value::Int32(value)) = row.get(column) else {
            return Err(EditError::Preservation);
        };
        original.push((*value, record.reference));
    }
    sort(&mut original, budget, cancelled)?;
    let base = index.root_page() as usize * PAGE_BYTES;
    let page = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let mut original_mask = [0_u8; MASK_BYTES];
    for (position, pair) in original.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, WIRE_BYTES as u64)
            .map_err(InspectError::from)?;
        let at = KEY_AT + position * WIRE_BYTES;
        if page.get(at..at + WIRE_BYTES) != Some(wire(*pair).as_slice()) {
            return Err(EditError::Preservation);
        }
        let end = (position + 1) * WIRE_BYTES;
        original_mask[end / 8] |= 1 << (end % 8);
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if page[MASK_AT..KEY_AT] != original_mask
        || usize::from(u16_at(page, FREE_AT)?) != PAGE_BYTES - KEY_AT - original.len() * WIRE_BYTES
    {
        return Err(EditError::Preservation);
    }

    let mut final_keys = super::vector(capacity, budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            (original.len() * size_of::<(i32, u32)>()) as u64,
        )
        .map_err(InspectError::from)?;
    final_keys.extend_from_slice(&original);
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        let Some(Value::Int32(value)) = request.values().get(column) else {
            return Err(EditError::Preservation);
        };
        final_keys.push((*value, *reference));
    }
    sort(&mut final_keys, budget, cancelled)?;
    if final_keys.len() > (PAGE_BYTES - KEY_AT) / WIRE_BYTES {
        return Err(EditError::Preservation);
    }
    let unique = index
        .constraints()
        .ok_or(InspectError::Unsupported)?
        .unique();
    let mut before_distinct = 0_usize;
    let mut last = None;
    for pair in &original {
        check(budget, cancelled)?;
        if last != Some(pair.0) {
            before_distinct += 1;
        }
        last = Some(pair.0);
    }
    let mut after_distinct = 0_usize;
    last = None;
    let mut mask = [0_u8; MASK_BYTES];
    for (position, pair) in final_keys.iter().enumerate() {
        check(budget, cancelled)?;
        if last == Some(pair.0) && unique {
            return Err(EditError::Preservation);
        }
        if last != Some(pair.0) {
            after_distinct += 1;
        }
        last = Some(pair.0);
        let at = base + KEY_AT + position * WIRE_BYTES;
        budget
            .charge(Resource::Steps, WIRE_BYTES as u64)
            .map_err(InspectError::from)?;
        expected[at..at + WIRE_BYTES].copy_from_slice(&wire(*pair));
        let end = (position + 1) * WIRE_BYTES;
        mask[end / 8] |= 1 << (end % 8);
    }
    budget
        .charge(Resource::Steps, (MASK_BYTES + size_of::<u16>()) as u64)
        .map_err(InspectError::from)?;
    expected[base + MASK_AT..base + KEY_AT].copy_from_slice(&mask);
    let free = u16::try_from(PAGE_BYTES - KEY_AT - final_keys.len() * WIRE_BYTES)
        .map_err(|_| InspectError::StructureLimit)?;
    expected[base + FREE_AT..base + FREE_AT + size_of::<u16>()]
        .copy_from_slice(&free.to_le_bytes());

    reconstruct_history(
        image,
        source,
        index,
        (before_distinct, after_distinct),
        expected,
        budget,
        cancelled,
    )
}

fn reconstruct_history(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    distinct: (usize, usize),
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let (before_distinct, after_distinct) = distinct;
    let original = inspect_table_index_counter(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        source.table.schema().name(),
        index.name(),
        budget,
        cancelled,
    )?;
    if original.definition_page() != source.table.schema().definition_page()
        || original.physical_index() != index.data_number()
    {
        return Err(EditError::Preservation);
    }
    let delta = after_distinct
        .checked_sub(before_distinct)
        .ok_or(EditError::Preservation)?;
    let delta = u32::try_from(delta).map_err(|_| InspectError::StructureLimit)?;
    let value = original
        .value()
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    let span = original.byte_range();
    if u32_at(image.as_bytes(), span.start)? != original.value() || span.len() != size_of::<u32>() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    expected[span].copy_from_slice(&value.to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}

fn sort(
    values: &mut [(i32, u32)],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let levels = values.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = values
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(size_of::<(i32, u32)>()))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    values.sort_unstable();
    check(budget, cancelled)?;
    Ok(())
}

fn wire((value, reference): (i32, u32)) -> [u8; WIRE_BYTES] {
    let mut encoded = [0_u8; WIRE_BYTES];
    encoded[0] = PRESENT_MARKER;
    encoded[1..5].copy_from_slice(&((value as u32) ^ SIGN_FLIP).to_be_bytes());
    encoded[5..9].copy_from_slice(&reference.to_be_bytes());
    encoded
}
