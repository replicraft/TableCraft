//! Literal native descriptor framing, independent of producer composition.
use crate::table::mutation::index_creation::buffer;
use crate::{EditError, InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
const PAGE: usize = 4096;
const FREE_OFFSET: usize = 2;
const COUNT_BLOCK_BYTES: usize = 12;
const COUNTER_OFFSET: usize = 4;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const INDEX_MAGIC: u32 = 1923;
const LOGICAL_MAGIC: u32 = 1625;
const KEY_START: usize = 4;
const KEY_COLUMN_SLOTS: usize = 10;
const KEY_SLOT_BYTES: usize = 3;
const KEY_DIRECTION_OFFSET: usize = 6;
const USAGE_OFFSET: usize = 34;
const ROOT_OFFSET: usize = 38;
const FLAGS_OFFSET: usize = 46;
const ORDINARY_FLAGS: u8 = 0x80;
const LOGICAL_NUMBER_OFFSET: usize = 4;
const PHYSICAL_NUMBER_OFFSET: usize = 8;
const RELATED_INDEX_OFFSET: usize = 13;
const DEFINITION_LENGTH_OFFSET: usize = 8;
const LOGICAL_COUNT_OFFSET: usize = 47;
const PHYSICAL_COUNT_OFFSET: usize = 51;
const DEFINITION_RESERVED_BYTES: usize = 8;

pub(super) fn definition(
    image: &NativeImage,
    original: &super::super::source::Original,
    root: u32,
    container: u32,
    counter: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let layout = &original.layout;
    let start = layout.definition as usize * PAGE;
    let raw = image
        .as_bytes()
        .get(start..start + PAGE)
        .ok_or(InspectError::Truncated)?;
    let bytes = original
        .name
        .as_str()
        .encode_utf16()
        .count()
        .checked_mul(2)
        .ok_or(InspectError::StructureLimit)?;
    let length = layout
        .length
        .checked_add(COUNT_BLOCK_BYTES + PHYSICAL_BYTES + LOGICAL_BYTES + size_of::<u16>())
        .and_then(|length| length.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    if length > PAGE {
        return Err(EditError::Preservation);
    }
    let mut active = buffer::vector(length, budget, cancelled)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    active.extend_from_slice(&raw[..layout.columns.start]);
    let mut counts = [0; COUNT_BLOCK_BYTES];
    integer(&mut counts, COUNTER_OFFSET, counter);
    active.extend(counts);
    // NO_CLONE: Read original column/physical/logical/name spans from a borrowed layout
    // during independent index frame reconstruction. Only range bounds are
    // copied, not the payload.
    active.extend_from_slice(&raw[layout.columns.clone()]);
    // NO_CLONE: Read original column/physical/logical/name spans from a borrowed layout
    // during independent index frame reconstruction. Only range bounds are
    // copied, not the payload.
    active.extend_from_slice(&raw[layout.physical.clone()]);
    let mut physical = [0; PHYSICAL_BYTES];
    integer(&mut physical, 0, INDEX_MAGIC);
    for key in 0..KEY_COLUMN_SLOTS {
        word(&mut physical, KEY_START + key * KEY_SLOT_BYTES, 0xffff);
    }
    word(
        &mut physical,
        KEY_START,
        u16::try_from(original.native).map_err(|_| InspectError::StructureLimit)?,
    );
    physical[KEY_DIRECTION_OFFSET] = u8::from(!original.descending);
    integer(
        &mut physical,
        USAGE_OFFSET,
        container
            .checked_mul(256)
            .ok_or(InspectError::StructureLimit)?,
    );
    integer(&mut physical, ROOT_OFFSET, root);
    physical[FLAGS_OFFSET] = ORDINARY_FLAGS;
    active.extend(physical);
    // NO_CLONE: Read original column/physical/logical/name spans from a borrowed layout
    // during independent index frame reconstruction. Only range bounds are
    // copied, not the payload.
    active.extend_from_slice(&raw[layout.logical.clone()]);
    let mut logical = [0; LOGICAL_BYTES];
    integer(&mut logical, 0, LOGICAL_MAGIC);
    integer(&mut logical, LOGICAL_NUMBER_OFFSET, original.logical);
    integer(&mut logical, PHYSICAL_NUMBER_OFFSET, layout.physical_count);
    integer(&mut logical, RELATED_INDEX_OFFSET, u32::MAX);
    active.extend(logical);
    // NO_CLONE: Read original column/physical/logical/name spans from a borrowed layout
    // during independent index frame reconstruction. Only range bounds are
    // copied, not the payload.
    active.extend_from_slice(&raw[layout.names.clone()]);
    active.extend(
        u16::try_from(bytes)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    for unit in original.name.as_str().encode_utf16() {
        check(budget, cancelled)?;
        active.extend(unit.to_le_bytes());
    }
    active.extend([0xff, 0xff]);
    if active.len() != length {
        return Err(EditError::Preservation);
    }
    word(
        &mut active,
        FREE_OFFSET,
        PAGE.saturating_sub(length + DEFINITION_RESERVED_BYTES) as u16,
    );
    integer(&mut active, DEFINITION_LENGTH_OFFSET, length as u32);
    integer(
        &mut active,
        LOGICAL_COUNT_OFFSET,
        layout
            .logical_count
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?,
    );
    integer(
        &mut active,
        PHYSICAL_COUNT_OFFSET,
        layout
            .physical_count
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?,
    );
    let mut expected = buffer::copy(raw, budget, cancelled)?;
    expected[..length].copy_from_slice(&active);
    Ok(expected)
}

fn word(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn integer(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
