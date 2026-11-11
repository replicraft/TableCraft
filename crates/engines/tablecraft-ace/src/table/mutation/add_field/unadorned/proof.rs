//! Literal TDEF reconstruction and complete source/row complement checks.
use super::super::{CurrentFieldAdditionError as Error, definition::Plan};
use crate::{
    InspectError, NativeImage, PropertyGroupKind, UnadornedFieldAdditionDefinition,
    header::check,
    read_table,
    table::mutation::{current_schema::Family, normal_fitted::binding},
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const PAGE: usize = 4096;
const COMPARE_BYTES: usize = 256;
const COLUMN_BYTES: usize = 25;
const MAGIC: u32 = 1625;
const MAGIC_AT: usize = 1;
const NUMBER_AT: usize = 5;
const VARIABLE_AT: usize = 7;
const ID_AT: usize = 9;
const FLAGS_AT: usize = 15;
const FIXED_AT: usize = 21;
const WIDTH_AT: usize = 23;
const ORDINARY_FIXED: u8 = 3;
const ORDINARY_VARIABLE: u8 = 2;
const VARIABLE_COUNT_AT: usize = 43;
const FREE_AT: usize = 2;
const LENGTH_AT: usize = 8;
const MAXIMUM_AT: usize = 41;
const ACTIVE_AT: usize = 45;
const DEFINITION_RESERVED: usize = 8;

pub(super) fn candidate(
    source: &super::unadorned_source::Source,
    plan: &Plan,
    definition: &UnadornedFieldAdditionDefinition,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    super::unadorned_source::qualify(source, definition.name(), budget, cancelled)?;
    let original = source.proof();
    let before = original.image();
    if after.len() != before.as_bytes().len() {
        return Err(Error::Preservation);
    }
    let old = &before.as_bytes()[plan.base..plan.base + PAGE];
    let new_length = plan
        .length
        .checked_add(COLUMN_BYTES + size_of::<u16>())
        .and_then(|length| {
            definition
                .name()
                .len()
                .checked_mul(size_of::<u16>())
                .and_then(|bytes| length.checked_add(bytes))
        })
        .filter(|length| *length <= PAGE - DEFINITION_RESERVED)
        .ok_or(Error::Preservation)?;
    let mut expected = Vec::new();
    budget.charge_allocation(PAGE as u64, 1)?;
    expected
        .try_reserve_exact(PAGE)
        .map_err(|_| InspectError::Allocation)?;
    budget.charge(Resource::Steps, (2 * PAGE) as u64)?;
    expected.extend_from_slice(&old[..plan.descriptor_end]);
    let (kind, width, fixed) = storage(definition)?;
    let mut descriptor = [0; COLUMN_BYTES];
    descriptor[0] = kind;
    descriptor[MAGIC_AT..MAGIC_AT + size_of::<u32>()].copy_from_slice(&MAGIC.to_le_bytes());
    word(&mut descriptor, NUMBER_AT, plan.number)?;
    word(&mut descriptor, VARIABLE_AT, plan.variable)?;
    word(&mut descriptor, ID_AT, usize::from(plan.column_id))?;
    descriptor[FLAGS_AT] = if fixed {
        ORDINARY_FIXED
    } else {
        ORDINARY_VARIABLE
    };
    if fixed && width != 0 {
        word(&mut descriptor, FIXED_AT, plan.fixed_offset)?;
    }
    word(&mut descriptor, WIDTH_AT, width)?;
    let column = definition.column();
    if kind == 10 {
        let order = crate::inspect_database_sort_order(&mut before.reader(), budget, cancelled)?;
        if order.general_family().is_none() {
            return Err(Error::Preservation);
        }
        descriptor[11..15].copy_from_slice(&order.native_bytes());
    }
    if kind == 16 {
        let constraints = column.decimal_constraints().ok_or(Error::Preservation)?;
        descriptor[11] = constraints.precision();
        descriptor[12] = constraints.scale();
    }
    expected.extend(descriptor);
    expected.extend_from_slice(&old[plan.descriptor_end..plan.index_start]);
    word_append(&mut expected, definition.name().len() * 2)?;
    for unit in definition.name().encode_utf16() {
        check(budget, cancelled)?;
        expected.extend(unit.to_le_bytes());
    }
    expected.extend_from_slice(&old[plan.index_start..plan.length]);
    let length = expected.len();
    if length != new_length {
        return Err(Error::Preservation);
    }
    word(&mut expected, FREE_AT, PAGE - length - DEFINITION_RESERVED)?;
    expected[LENGTH_AT..LENGTH_AT + size_of::<u32>()]
        .copy_from_slice(&(length as u32).to_le_bytes());
    word(&mut expected, MAXIMUM_AT, plan.number + 1)?;
    word(&mut expected, ACTIVE_AT, plan.active_count + 1)?;
    word(
        &mut expected,
        VARIABLE_COUNT_AT,
        plan.variable_high_water + usize::from(!fixed),
    )?;
    expected.extend_from_slice(&old[length..]);
    if expected.len() != PAGE || after.get(plan.base..plan.base + PAGE) != Some(expected.as_slice())
    {
        return Err(Error::Preservation);
    }
    for (page, (left, right)) in before
        .as_bytes()
        .chunks(PAGE)
        .zip(after.chunks(PAGE))
        .enumerate()
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, PAGE as u64)?;
        if page * PAGE != plan.base && left != right {
            return Err(Error::Preservation);
        }
    }
    let scope = before.catalog().catalog().scope();
    let checked = NativeImage::capture_retained(&mut Cursor::new(after), scope, budget, cancelled)?;
    let observed =
        Family::Retained.observe(&checked, original.table().schema.name(), budget, cancelled)?;
    super::unadorned_source::prove_preserved(source, &checked, &observed, budget, cancelled)?;
    for group in observed.properties.groups() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            group.name().len().max(definition.name().len()) as u64,
        )?;
        if group.kind() == PropertyGroupKind::Column
            && group.name().eq_ignore_ascii_case(definition.name())
        {
            return Err(Error::Preservation);
        }
    }
    if !equal_payload(
        observed.properties.raw_bytes(),
        original.table().properties.raw_bytes(),
        budget,
        cancelled,
    )? || !equal_payload(
        observed.database.raw_bytes(),
        original.table().database.raw_bytes(),
        budget,
        cancelled,
    )? || observed.schema.columns().len() != plan.active_count + 1
    {
        return Err(Error::Preservation);
    }
    let old_rows = read_table(
        &mut before.reader(),
        scope,
        original.table().schema.name(),
        budget,
        cancelled,
    )?;
    let new_rows = read_table(
        &mut checked.reader(),
        scope,
        original.table().schema.name(),
        budget,
        cancelled,
    )?;
    if old_rows.rows().len() != new_rows.rows().len() {
        return Err(Error::Preservation);
    }
    let missing = if column.scalar_type() == ScalarType::Boolean {
        Value::Boolean(false)
    } else {
        Value::Null
    };
    for (old, new) in old_rows.rows().iter().zip(new_rows.rows()) {
        check(budget, cancelled)?;
        if new.len() != old.len() + 1 {
            return Err(Error::Preservation);
        }
        for (left, right) in old.iter().zip(new) {
            if !equal_value(left, right, budget, cancelled)? {
                return Err(Error::Preservation);
            }
        }
        if !equal_value(
            new.last().ok_or(Error::Preservation)?,
            &missing,
            budget,
            cancelled,
        )? {
            return Err(Error::Preservation);
        }
    }
    Ok(())
}

// Value equality is representational here: no SQL promotion, collation, or
// temporal normalization belongs in a preservation proof.
fn equal_value(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    match (left, right) {
        (Value::Text(left), Value::Text(right)) => {
            equal_payload(left.as_bytes(), right.as_bytes(), budget, cancelled)
        }
        (Value::Binary(left), Value::Binary(right)) => {
            equal_payload(left, right, budget, cancelled)
        }
        // All remaining stored carriers are bounded scalar representations;
        // mismatched variants also have no payload comparison.
        _ => Ok(left == right),
    }
}

fn equal_payload(
    left: &[u8],
    right: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, 1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.chunks(COMPARE_BYTES).zip(right.chunks(COMPARE_BYTES)) {
        if !binding::equal_bytes(left, right, budget, cancelled)? {
            return Ok(false);
        }
    }
    Ok(true)
}

// Literal independent native tags/widths; do not call the producer profile map.
fn storage(definition: &UnadornedFieldAdditionDefinition) -> Result<(u8, usize, bool), Error> {
    let column = definition.column();
    Ok(match column.scalar_type() {
        ScalarType::Boolean => (1, 0, true),
        ScalarType::Byte => (2, 1, true),
        ScalarType::Int16 => (3, 2, true),
        ScalarType::Int32 => (4, 4, true),
        ScalarType::Int64 => (19, 8, true),
        ScalarType::Single => (6, 4, true),
        ScalarType::Double => (7, 8, true),
        ScalarType::Currency => (5, 8, true),
        ScalarType::DateTime => (8, 8, true),
        ScalarType::Guid => (15, 16, true),
        ScalarType::Text => (
            10,
            column.max_text_utf16_units().ok_or(Error::Preservation)? * 2,
            false,
        ),
        ScalarType::Decimal => (16, 17, false),
        _ => return Err(Error::Preservation),
    })
}

fn word(bytes: &mut [u8], offset: usize, value: usize) -> Result<(), Error> {
    let value = u16::try_from(value).map_err(|_| Error::Capacity)?;
    bytes
        .get_mut(offset..offset + 2)
        .ok_or(Error::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn word_append(bytes: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    bytes.extend(
        u16::try_from(value)
            .map_err(|_| Error::Capacity)?
            .to_le_bytes(),
    );
    Ok(())
}
