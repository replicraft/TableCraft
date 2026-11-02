//! Paid source/request overflow bounds choose one route before physical planning.
use crate::{
    EditError, InspectError, NativeImage, RowInsert,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    limits::MAX_LIVE_ROWS,
    table::{
        indexes::{
            header::IndexHeader,
            page::{ENTRY_START, LEAF},
        },
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const MANUAL_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG;
const PAGE_VERSION: u8 = 1;
const PRESENT: u8 = 0x7f;
const KEY_BYTES: usize = size_of::<u8>() + size_of::<i64>();
const WIRE_BYTES: usize = KEY_BYTES + size_of::<u32>();

pub(super) fn units(value: &Value, money: bool) -> Option<i64> {
    match (money, value) {
        (false, Value::Int64(value)) => Some(*value),
        (true, Value::Currency(value)) => Some(value.scaled()),
        _ => None,
    }
}

pub(super) fn shape(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let schema = source.table.schema();
    if schema.layout.physical_indexes != 3
        || schema.columns().len() != 3
        || source.records.is_empty()
    {
        return Ok(false);
    }
    let fields = schema.columns();
    if fields[0].scalar_type() != Some(ScalarType::Int32)
        || fields[2].scalar_type() != Some(ScalarType::Int32)
        || !matches!(
            fields[1].scalar_type(),
            Some(ScalarType::Int64 | ScalarType::Currency)
        )
    {
        return Ok(false);
    }
    for (position, field) in fields.iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position
            || field.flags() != MANUAL_FLAGS
            || field.extended_flags() != 0
            || !field.fixed()
            || field.storage_length()
                != [size_of::<i32>(), size_of::<i64>(), size_of::<i32>()][position]
        {
            return Ok(false);
        }
    }
    for physical in 0..3 {
        let mut found = false;
        for index in source.indexes.indexes() {
            check(budget, cancelled)?;
            if index.data_number() as usize != physical {
                continue;
            }
            let [key] = index.columns() else {
                return Ok(false);
            };
            if key.number() != usize::from(physical != 0) || key.descending() != (physical == 2) {
                return Ok(false);
            }
            let Some(constraints) = index.constraints() else {
                return Ok(false);
            };
            if constraints.ignore_nulls()
                || (physical == 0
                    && (!constraints.primary_key()
                        || !constraints.unique()
                        || !constraints.required()))
                || (physical != 0 && constraints.primary_key())
            {
                return Ok(false);
            }
            let at = index.root_page() as usize * PAGE_BYTES;
            let raw = image
                .as_bytes()
                .get(at..at + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            let header = IndexHeader::read(raw)?;
            if (physical == 0 && header.prefix.get() != 0)
                || header.kind != LEAF
                || header.version != PAGE_VERSION
                || header.owner.get() != schema.definition_page()
                || !header.qualified_for_update(0)
                || header.previous.get() != 0
                || header.next.get() != 0
                || header.tail.get() != 0
            {
                return Ok(false);
            }
            found = true;
        }
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(in crate::table::mutation::normal_insert) fn selected(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if requests.is_empty() || source.table.schema().layout.physical_indexes != 3 {
        return Ok(false);
    }
    if !shape(image, source, budget, cancelled)? {
        return Ok(false);
    }
    let count = source
        .records
        .len()
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    let money = source.table.schema().columns()[1].scalar_type() == Some(ScalarType::Currency);
    let mut first: Option<[u8; KEY_BYTES]> = None;
    let mut common = KEY_BYTES;
    for row in source
        .table
        .rows()
        .iter()
        .map(Vec::as_slice)
        .chain(requests.iter().map(RowInsert::values))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * KEY_BYTES) as u64)
            .map_err(InspectError::from)?;
        let Some(value) = row.get(1).and_then(|value| units(value, money)) else {
            return Ok(false);
        };
        let mut encoded = [0; KEY_BYTES];
        encoded[0] = PRESENT;
        encoded[1..].copy_from_slice(&((value as u64) ^ (1 << 63)).to_be_bytes());
        if let Some(first) = first {
            common = encoded[..common]
                .iter()
                .zip(&first[..common])
                .take_while(|(left, right)| left == right)
                .count();
        } else {
            first = Some(encoded);
        }
    }
    // Even maximally sharing the whole common key cannot fit this population.
    let minimum = count
        .checked_mul(WIRE_BYTES)
        .and_then(|total| {
            common
                .checked_mul(count - 1)
                .and_then(|saved| total.checked_sub(saved))
        })
        .ok_or(InspectError::StructureLimit)?;
    Ok(minimum > PAGE_BYTES - ENTRY_START)
}
