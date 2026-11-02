//! Original scalar population and manual signed component/ROW encoding.
use super::super::vector;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeIndex,
    header::check,
    table::mutation::{fixed_records, preserved_source::Source},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const ROW_BYTES: usize = size_of::<u32>();
const ASC_PRESENT: u8 = 0x7f;
const DESC_PRESENT: u8 = 0x80;
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;

pub(super) struct Population {
    pub original: Vec<Vec<u8>>,
    pub final_keys: Vec<Vec<u8>>,
}

fn units(kind: ScalarType, value: &Value) -> Result<Option<i64>, EditError> {
    match (kind, value) {
        (ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency, Value::Null) => Ok(None),
        (ScalarType::Int32, Value::Int32(value)) => Ok(Some(i64::from(*value))),
        (ScalarType::Int64, Value::Int64(value)) => Ok(Some(*value)),
        (ScalarType::Currency, Value::Currency(value)) => Ok(Some(value.scaled())),
        _ => Err(EditError::Preservation),
    }
}

fn requested<'a>(
    source: &Source,
    groups: &'a [FixedColumnUpdates<'_>],
    row: usize,
    column: usize,
    original: &'a Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a Value, EditError> {
    let name = source.table.schema().columns()[column].name();
    let mut selected = original;
    for group in groups {
        check(budget, cancelled)?;
        fixed_records::name_work(name, group.column(), budget)?;
        if !name.eq_ignore_ascii_case(group.column()) {
            continue;
        }
        budget
            .charge(Resource::Steps, group.updates().len() as u64)
            .map_err(InspectError::from)?;
        if let Some(update) = group.updates().iter().find(|update| update.row() == row) {
            budget
                .charge(Resource::Steps, (2 * size_of::<i64>()) as u64)
                .map_err(InspectError::from)?;
            if original != update.expected() {
                return Err(EditError::Conflict);
            }
            selected = update.value();
        }
    }
    Ok(selected)
}

fn wire(
    value: Option<i64>,
    width: usize,
    descending: bool,
    row: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let capacity = size_of::<u8>() + width + ROW_BYTES;
    let mut bytes = vector(capacity, budget, cancelled)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    if let Some(value) = value {
        bytes.push(if descending {
            DESC_PRESENT
        } else {
            ASC_PRESENT
        });
        let encoded = match width {
            width if width == size_of::<i32>() => {
                u64::from((value as u32) ^ 0x8000_0000).to_be_bytes()
            }
            width if width == size_of::<i64>() => {
                ((value as u64) ^ 0x8000_0000_0000_0000).to_be_bytes()
            }
            _ => return Err(EditError::Preservation),
        };
        for byte in &encoded[encoded.len() - width..] {
            bytes.push(if descending { !*byte } else { *byte });
        }
    } else {
        bytes.push(if descending { DESC_NULL } else { ASC_NULL });
    }
    bytes.extend_from_slice(&row.to_be_bytes());
    Ok(bytes)
}

fn sorted(
    keys: &mut [Vec<u8>],
    maximum: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let work = keys
        .len()
        .checked_mul(keys.len().checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|count| count.checked_mul(maximum))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable();
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn capture(
    source: &Source,
    selected: &NativeIndex,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Population, EditError> {
    let [key] = selected.columns() else {
        return Err(EditError::Preservation);
    };
    let column = key.number();
    let field = source
        .table
        .schema()
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    let kind = field.scalar_type().ok_or(EditError::Preservation)?;
    let width = field.storage_length();
    let mut original = vector(source.records.len(), budget, cancelled)?;
    let mut final_keys = vector(source.records.len(), budget, cancelled)?;
    let flags = selected.constraints().ok_or(EditError::Preservation)?;
    for (ordinal, record) in source.records.iter().enumerate() {
        check(budget, cancelled)?;
        let value = source
            .table
            .rows()
            .get(ordinal)
            .and_then(|row| row.get(column))
            .ok_or(EditError::Preservation)?;
        original.push(wire(
            units(kind, value)?,
            width,
            key.descending(),
            record.reference,
            budget,
            cancelled,
        )?);
        let replacement = requested(source, groups, ordinal, column, value, budget, cancelled)?;
        let final_value = units(kind, replacement)?;
        if flags.required() && final_value.is_none() {
            return Err(EditError::Preservation);
        }
        final_keys.push(wire(
            final_value,
            width,
            key.descending(),
            record.reference,
            budget,
            cancelled,
        )?);
    }
    let maximum = width + size_of::<u8>() + ROW_BYTES;
    sorted(&mut original, maximum, budget, cancelled)?;
    sorted(&mut final_keys, maximum, budget, cancelled)?;
    if flags.unique() {
        for pair in final_keys.windows(2) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (2 * maximum) as u64)
                .map_err(InspectError::from)?;
            if pair[0].len() != size_of::<u8>() + ROW_BYTES
                && pair[0][..pair[0].len() - ROW_BYTES] == pair[1][..pair[1].len() - ROW_BYTES]
            {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(Population {
        original,
        final_keys,
    })
}
