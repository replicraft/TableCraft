//! Repack only the existing definition/name and live user/OFF property rows.
use super::super::{FieldRenameError as Error, source::Qualified};
use super::layout::{FREE_OFFSET, Layout};
use crate::{
    InspectError,
    format::{bytes::u32_at, definition::LENGTH_OFFSET, page::PAGE_BYTES, row::slot_offset},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_BYTES: usize = 16 * 1024;
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

pub(super) fn candidate(
    source: &Qualified,
    layout: &Layout,
    new_name: &str,
    delta: isize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let before = source.image().as_bytes();
    budget.charge_allocation(before.len() as u64, 1)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(before.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.chunks(COPY_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        after.extend_from_slice(chunk);
    }

    let observed = &source.observed;
    let definition_end = layout.definition_base + layout.definition_length;
    let name_end = shifted(observed.definition_name.end, delta)?;
    budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
    after.copy_within(observed.definition_name.end..definition_end, name_end);
    write_name(&mut after, observed.definition_name.start, new_name)?;
    short(
        &mut after,
        observed.definition_name.start - size_of::<u16>(),
        new_name.len() * size_of::<u16>(),
    )?;
    long(
        &mut after,
        layout.definition_base + LENGTH_OFFSET,
        shifted(layout.definition_length, delta)?,
    )?;
    short(
        &mut after,
        layout.definition_base + FREE_OFFSET,
        shifted(layout.definition_free, -delta)?,
    )?;

    let original = observed.properties.raw_bytes();
    let name_start = observed.property_name.start - observed.property_body.start;
    let name_end = observed.property_name.end - observed.property_body.start;
    let body_bytes = shifted(original.len(), delta)?;
    budget.charge_allocation(body_bytes as u64, 1)?;
    budget.charge(Resource::Steps, (original.len() + body_bytes) as u64)?;
    let mut body = Vec::new();
    body.try_reserve_exact(body_bytes)
        .map_err(|_| InspectError::Allocation)?;
    body.extend_from_slice(&original[..name_start]);
    for unit in new_name.encode_utf16() {
        body.extend_from_slice(&unit.to_le_bytes());
    }
    body.extend_from_slice(&original[name_end..]);
    long(
        &mut body,
        layout.group.start,
        shifted(layout.group.len(), delta)?,
    )?;
    let name_block = layout.group.start + GROUP_HEADER_BYTES;
    long(
        &mut body,
        name_block,
        shifted(u32_at(original, name_block)? as usize, delta)?,
    )?;
    short(
        &mut body,
        name_start - size_of::<u16>(),
        new_name.len() * size_of::<u16>(),
    )?;

    let user_start = shifted(layout.property_start, -delta)?;
    let database_start = shifted(layout.database_start, -delta)?;
    let base = layout.property_base;
    budget.charge(Resource::Steps, PAGE_BYTES as u64)?;
    after.copy_within(
        base + layout.database_start..base + layout.property_start,
        base + database_start,
    );
    after[base + user_start..base + layout.property_end].copy_from_slice(&body);
    short(
        &mut after,
        base + slot_offset(layout.property_slot),
        user_start,
    )?;
    short(
        &mut after,
        base + slot_offset(layout.property_slot + 1),
        database_start,
    )?;
    short(
        &mut after,
        base + FREE_OFFSET,
        shifted(layout.free, -delta)?,
    )?;

    let descriptor = observed.property_descriptor.start;
    long(
        &mut after,
        descriptor,
        shifted(u32_at(before, descriptor)? as usize, delta)?,
    )?;
    check(budget, cancelled)?;
    Ok(after)
}

fn write_name(bytes: &mut [u8], start: usize, name: &str) -> Result<(), Error> {
    let output = bytes
        .get_mut(start..start + name.len() * size_of::<u16>())
        .ok_or(Error::Preservation)?;
    for (word, unit) in output
        .as_chunks_mut::<{ size_of::<u16>() }>()
        .0
        .iter_mut()
        .zip(name.encode_utf16())
    {
        word.copy_from_slice(&unit.to_le_bytes());
    }
    Ok(())
}
