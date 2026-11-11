//! Literal TDEF reconstruction and complete source/row complement checks.
use super::super::{CurrentFieldAdditionError as Error, definition::Plan};
use crate::{
    DeclaredFieldAdditionDefinition, InspectError, NativeImage, PropertyGroupKind,
    header::check,
    read_table,
    table::mutation::current_schema::{self, Family},
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const PAGE: usize = 4096;
const COLUMN_BYTES: usize = 25;
const CURRENCY: u8 = 5;
const CURRENCY_BYTES: usize = 8;
const MAGIC: u32 = 1625;
const MAGIC_AT: usize = 1;
const NUMBER_AT: usize = 5;
const VARIABLE_AT: usize = 7;
const ID_AT: usize = 9;
const FLAGS_AT: usize = 15;
const FIXED_AT: usize = 21;
const WIDTH_AT: usize = 23;
const ORDINARY_FIXED: u8 = 3;
const FREE_AT: usize = 2;
const LENGTH_AT: usize = 8;
const MAXIMUM_AT: usize = 41;
const ACTIVE_AT: usize = 45;
const DEFINITION_RESERVED: usize = 8;

pub(super) fn candidate(
    source: &current_schema::Source,
    plan: &Plan,
    definition: &DeclaredFieldAdditionDefinition,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    super::qualify_source(source, definition, budget, cancelled)?;
    let before = source.image();
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
    let mut descriptor = [0; COLUMN_BYTES];
    descriptor[0] = CURRENCY;
    descriptor[MAGIC_AT..MAGIC_AT + size_of::<u32>()].copy_from_slice(&MAGIC.to_le_bytes());
    word(&mut descriptor, NUMBER_AT, plan.number)?;
    word(&mut descriptor, VARIABLE_AT, plan.variable)?;
    word(&mut descriptor, ID_AT, usize::from(plan.column_id))?;
    descriptor[FLAGS_AT] = ORDINARY_FIXED;
    word(&mut descriptor, FIXED_AT, plan.fixed_offset)?;
    word(&mut descriptor, WIDTH_AT, CURRENCY_BYTES)?;
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
        Family::Retained.observe(&checked, source.table().schema.name(), budget, cancelled)?;
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
    if observed.properties.raw_bytes() != source.table().properties.raw_bytes()
        || observed.database.raw_bytes() != source.table().database.raw_bytes()
        || observed.schema.columns().len() != plan.active_count + 1
    {
        return Err(Error::Preservation);
    }
    let old_rows = read_table(
        &mut before.reader(),
        scope,
        source.table().schema.name(),
        budget,
        cancelled,
    )?;
    let new_rows = read_table(
        &mut checked.reader(),
        scope,
        source.table().schema.name(),
        budget,
        cancelled,
    )?;
    if old_rows.rows().len() != new_rows.rows().len() {
        return Err(Error::Preservation);
    }
    for (old, new) in old_rows.rows().iter().zip(new_rows.rows()) {
        check(budget, cancelled)?;
        if new.len() != old.len() + 1
            || &new[..old.len()] != old
            || new.last() != Some(&Value::Null)
        {
            return Err(Error::Preservation);
        }
    }
    Ok(())
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
