//! Fitting name frames retain all original declaration and property snapshots.
use super::{Error, source::Source};
use crate::{
    InspectError,
    format::{
        bytes::u32_at,
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        page::PAGE_BYTES,
    },
    header::check,
    properties::framing,
    table::mutation::{record_serialization, saved_field::Observed},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;
const GROUP_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();

pub(super) fn shifted(value: usize, delta: isize) -> Result<usize, Error> {
    value.checked_add_signed(delta).ok_or(Error::Preservation)
}

pub(super) fn short(bytes: &mut [u8], at: usize, value: usize) -> Result<(), Error> {
    let value = u16::try_from(value).map_err(|_| Error::Preservation)?;
    bytes
        .get_mut(at..at + size_of::<u16>())
        .ok_or(Error::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) fn long(bytes: &mut [u8], at: usize, value: usize) -> Result<(), Error> {
    let value = u32::try_from(value).map_err(|_| Error::Preservation)?;
    bytes
        .get_mut(at..at + size_of::<u32>())
        .ok_or(Error::Preservation)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub(super) fn owned_name(name: &str, budget: &mut ResourceBudget) -> Result<String, Error> {
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut result = String::new();
    result
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(name);
    Ok(result)
}

pub(super) fn definition(
    source: &Source,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let original = source.image.as_bytes();
    let mut after = record_serialization::copy(original, budget, cancelled)?;
    let observed = &source.observed;
    if new_name == observed.column().name() {
        return Ok(after);
    }

    let delta = (new_name.len() as isize - observed.identity.name_units as isize)
        * size_of::<u16>() as isize;
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let length = u32_at(original, base + LENGTH_OFFSET)? as usize;
    let end = shifted(observed.definition_name.end, delta)?;
    budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
    check(budget, cancelled)?;
    after.copy_within(observed.definition_name.end..base + length, end);
    for (index, unit) in new_name.encode_utf16().enumerate() {
        short(
            &mut after,
            observed.definition_name.start + index * size_of::<u16>(),
            usize::from(unit),
        )?;
    }
    short(
        &mut after,
        observed.definition_name.start - size_of::<u16>(),
        new_name.len() * size_of::<u16>(),
    )?;
    long(&mut after, base + LENGTH_OFFSET, shifted(length, delta)?)?;
    short(
        &mut after,
        base + FREE_OFFSET,
        PAGE_BYTES - shifted(length, delta)? - PAGE_HEADER_BYTES,
    )?;
    Ok(after)
}

pub(super) fn properties(
    observed: &Observed,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let raw = observed.properties.raw_bytes();
    let start = observed
        .property_name
        .start
        .checked_sub(observed.property_body.start)
        .ok_or(Error::Preservation)?;
    let end = observed
        .property_name
        .end
        .checked_sub(observed.property_body.start)
        .ok_or(Error::Preservation)?;
    let delta = (new_name.len() as isize - observed.identity.name_units as isize)
        * size_of::<u16>() as isize;
    let mut selected = None;
    framing::current_schema_group_blocks(
        &observed.properties,
        budget,
        cancelled,
        |group, span| {
            if group == observed.group && selected.replace(span).is_some() {
                return Err(InspectError::Malformed);
            }
            Ok(())
        },
    )?;
    let group = selected.ok_or(Error::Source)?;
    let length = shifted(raw.len(), delta)?;
    budget.charge_allocation(length as u64, 1)?;
    budget.charge(Resource::Steps, length as u64)?;
    let mut body = Vec::new();
    body.try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    body.extend_from_slice(&raw[..start]);
    for unit in new_name.encode_utf16() {
        body.extend_from_slice(&unit.to_le_bytes());
    }
    body.extend_from_slice(&raw[end..]);
    long(&mut body, group.start, shifted(group.len(), delta)?)?;
    let name_frame = group.start + GROUP_HEADER_BYTES;
    long(
        &mut body,
        name_frame,
        shifted(u32_at(raw, name_frame)? as usize, delta)?,
    )?;
    short(
        &mut body,
        start - size_of::<u16>(),
        new_name.len() * size_of::<u16>(),
    )?;
    Ok(body)
}
