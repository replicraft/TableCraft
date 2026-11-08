//! Reconstruct the closed new-definition page independently of its serializer.
use super::super::{AddTableError, geometry::Role, source};
use super::width;
use crate::{CreateColumn, FirstTableDefinition, InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const PAGE_BYTES: usize = 4096;
const FIELD_START: usize = 75;
const FIELD_BYTES: usize = 25;
const PHYSICAL_INDEX_BYTES: usize = 52;
const LOGICAL_INDEX_BYTES: usize = 28;
const KEY_SLOTS: usize = 10;
const KEY_BYTES: usize = 3;
const MAP_PAGE: u32 = 3;

fn short(bytes: &mut [u8], at: usize, value: usize) -> Result<(), AddTableError> {
    let value = u16::try_from(value).map_err(|_| AddTableError::Preservation)?;
    bytes
        .get_mut(at..at + 2)
        .ok_or(AddTableError::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn long(bytes: &mut [u8], at: usize, value: u32) -> Result<(), AddTableError> {
    bytes
        .get_mut(at..at + 4)
        .ok_or(AddTableError::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn name(
    bytes: &mut [u8],
    at: &mut usize,
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    short(bytes, *at, text.encode_utf16().count() * 2)?;
    *at += 2;
    for unit in text.encode_utf16() {
        check(budget, cancelled)?;
        short(bytes, *at, usize::from(unit))?;
        *at += 2;
    }
    Ok(())
}

fn kind(scalar: ScalarType) -> Result<u8, AddTableError> {
    Ok(match scalar {
        ScalarType::Boolean => 1,
        ScalarType::Byte => 2,
        ScalarType::Int16 => 3,
        ScalarType::Int32 => 4,
        ScalarType::Currency => 5,
        ScalarType::Single => 6,
        ScalarType::Double => 7,
        ScalarType::DateTime => 8,
        ScalarType::Binary => 9,
        ScalarType::Text => 10,
        ScalarType::Guid => 15,
        ScalarType::Decimal => 16,
        ScalarType::Int64 => 19,
        ScalarType::DateTimeExtended => return Err(AddTableError::Preservation),
    })
}

pub(super) fn verify(
    after: &[u8],
    role: Role,
    definition: &FirstTableDefinition,
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    check(budget, cancelled)?;
    budget
        // Bound zero-initialization, field/name reconstruction and comparison,
        // independently of the cancellation polls inside field traversal.
        .charge(Resource::Steps, (4 * PAGE_BYTES) as u64)
        .map_err(InspectError::from)?;
    let mut expected = [0; PAGE_BYTES];
    let mut names = FIELD_START + columns.len() * FIELD_BYTES;
    let mut fixed_offset = 0;
    let mut variable = 0;
    for (position, column) in columns.iter().enumerate() {
        check(budget, cancelled)?;
        let at = FIELD_START + position * FIELD_BYTES;
        let scalar = column.scalar_type();
        let variable_field = matches!(
            scalar,
            ScalarType::Text | ScalarType::Binary | ScalarType::Decimal
        );
        let field_width = width(*column)?;
        let offset = if variable_field || scalar == ScalarType::Boolean {
            0
        } else {
            fixed_offset
        };
        expected[at] = kind(scalar)?;
        long(&mut expected, at + 1, 1625)?;
        short(&mut expected, at + 5, position)?;
        short(&mut expected, at + 7, variable)?;
        short(&mut expected, at + 9, position)?;
        if scalar == ScalarType::Text {
            expected[at + 11..at + 15].copy_from_slice(&[9, 4, 0, 1]);
        } else if let Some(decimal) = column.decimal_constraints() {
            expected[at + 11] = decimal.precision();
            expected[at + 12] = decimal.scale();
        }
        expected[at + 15] = if position == 0 {
            7
        } else if variable_field {
            2
        } else {
            3
        };
        short(&mut expected, at + 21, offset)?;
        short(&mut expected, at + 23, field_width)?;
        name(&mut expected, &mut names, column.name(), budget, cancelled)?;
        if variable_field {
            variable += 1;
        } else {
            fixed_offset += field_width;
        }
    }

    let physical = names;
    long(&mut expected, physical, 1923)?;
    for key in 0..KEY_SLOTS {
        short(
            &mut expected,
            physical + 4 + key * KEY_BYTES,
            usize::from(u16::MAX),
        )?;
    }
    short(&mut expected, physical + 4, 0)?;
    expected[physical + 6] = 1;
    long(
        &mut expected,
        physical + 34,
        (MAP_PAGE << 8) | role.index as u32,
    )?;
    long(&mut expected, physical + 38, role.root as u32)?;
    expected[physical + 46] = 0x89;
    names += PHYSICAL_INDEX_BYTES;
    long(&mut expected, names, 1625)?;
    long(&mut expected, names + 13, u32::MAX)?;
    expected[names + 23] = 1;
    names += LOGICAL_INDEX_BYTES;
    name(
        &mut expected,
        &mut names,
        definition.primary_name(),
        budget,
        cancelled,
    )?;
    short(&mut expected, names, usize::from(u16::MAX))?;
    let length = names + 2;

    expected[..2].copy_from_slice(&[2, 1]);
    short(
        &mut expected,
        2,
        PAGE_BYTES
            .checked_sub(length + 8)
            .ok_or(AddTableError::Preservation)?,
    )?;
    long(&mut expected, 8, length as u32)?;
    long(&mut expected, 12, 1625)?;
    expected[24] = 1;
    expected[40] = 0x4e;
    short(&mut expected, 41, columns.len())?;
    short(&mut expected, 43, variable)?;
    short(&mut expected, 45, columns.len())?;
    long(&mut expected, 47, 1)?;
    long(&mut expected, 51, 1)?;
    long(&mut expected, 55, (MAP_PAGE << 8) | role.owned as u32)?;
    long(&mut expected, 59, (MAP_PAGE << 8) | role.candidate as u32)?;
    if source::page(after, role.definition) != expected {
        return Err(AddTableError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
