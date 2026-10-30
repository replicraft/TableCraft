//! Independent byte framing for the final certificate, not producer page helpers.
use super::super::buffer;
use crate::{EditError, InspectError, NativeImage, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PAGE: usize = 4096;
const START: usize = 480;
const CAPACITY: usize = PAGE - START;

const FREE_OFFSET: usize = 2;
const OWNER_OFFSET: usize = 4;
const PREVIOUS_OFFSET: usize = 12;
const NEXT_OFFSET: usize = 16;
const TAIL_OFFSET: usize = 20;
const PREFIX_OFFSET: usize = 24;
const HEIGHT_OFFSET: usize = 26;
const END_BITMAP_OFFSET: usize = 27;
const LEAF_KIND: u8 = 4;
const NODE_KIND: u8 = 3;
const PAGE_VERSION: u8 = 1;
const MAP_RECORD_BYTES: usize = 69;
const MAP_BITMAP_OFFSET: usize = 5;
const DATA_ROW_COUNT_OFFSET: usize = 12;
const DATA_SLOT_OFFSET: usize = 14;
const DATA_DIRECTORY_BYTES: usize = 16;
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

pub(super) fn fits(
    wires: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    Ok(packed(wires, budget, cancelled)?.1 <= CAPACITY)
}

fn packed(
    wires: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), InspectError> {
    let Some(first) = wires.first() else {
        return Ok((0, 0));
    };
    let mut common = first.len();
    let mut total = 0_usize;
    for wire in wires {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, wire.len() as u64)?;
        total = total
            .checked_add(wire.len())
            .ok_or(InspectError::StructureLimit)?;
        common = first[..common]
            .iter()
            .zip(wire)
            .take_while(|(left, right)| left == right)
            .count();
    }
    let prefix = if total > CAPACITY { common } else { 0 };
    let saved = prefix
        .checked_mul(wires.len().saturating_sub(1))
        .ok_or(InspectError::StructureLimit)?;
    Ok((
        prefix,
        total.checked_sub(saved).ok_or(InspectError::Malformed)?,
    ))
}

pub(super) fn index_page(
    owner: u32,
    links: (u32, u32),
    tail: u32,
    height: u8,
    wires: &[Vec<u8>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let (prefix, active) = packed(wires, budget, cancelled)?;
    if active > CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut page = buffer::vector(PAGE, budget, cancelled)?;
    budget
        .charge(Resource::Steps, PAGE as u64)
        .map_err(InspectError::from)?;
    page.resize(PAGE, 0);
    page[0] = if height == 0 { LEAF_KIND } else { NODE_KIND };
    page[1] = PAGE_VERSION;
    word(&mut page, FREE_OFFSET, (CAPACITY - active) as u16);
    integer(&mut page, OWNER_OFFSET, owner);
    integer(&mut page, PREVIOUS_OFFSET, links.0);
    integer(&mut page, NEXT_OFFSET, links.1);
    integer(&mut page, TAIL_OFFSET, tail);
    word(&mut page, PREFIX_OFFSET, prefix as u16);
    page[HEIGHT_OFFSET] = height;
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        check(budget, cancelled)?;
        let shared = if position == 0 { 0 } else { prefix };
        let suffix = &wire[shared..];
        page[START + end..START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[END_BITMAP_OFFSET + end / 8] |= 1 << (end % 8);
    }
    if end != active {
        return Err(EditError::Preservation);
    }
    Ok(page)
}

pub(super) fn map_page(
    root: u32,
    bodies: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut page = buffer::vector(PAGE, budget, cancelled)?;
    budget.charge(Resource::Steps, PAGE as u64)?;
    page.resize(PAGE, 0);
    page[..2].copy_from_slice(&[1, 1]);
    let start = PAGE - MAP_RECORD_BYTES;
    word(
        &mut page,
        FREE_OFFSET,
        (start - DATA_DIRECTORY_BYTES) as u16,
    );
    word(&mut page, DATA_ROW_COUNT_OFFSET, 1);
    word(&mut page, DATA_SLOT_OFFSET, start as u16);
    integer(&mut page, start + 1, root);
    for bit in 0..bodies {
        check(budget, cancelled)?;
        page[start + MAP_BITMAP_OFFSET + bit / 8] |= 1 << (bit % 8);
    }
    Ok(page)
}

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
    active.extend_from_slice(&raw[layout.columns.clone()]);
    active.extend_from_slice(&raw[layout.physical.clone()]);
    let mut physical = [0; PHYSICAL_BYTES];
    integer(&mut physical, 0, INDEX_MAGIC);
    for key in 0..KEY_COLUMN_SLOTS {
        word(&mut physical, KEY_START + key * KEY_SLOT_BYTES, 0xffff);
    }
    word(
        &mut physical,
        KEY_START,
        u16::try_from(original.column).map_err(|_| InspectError::StructureLimit)?,
    );
    physical[KEY_DIRECTION_OFFSET] = u8::from(!original.descending);
    integer(&mut physical, USAGE_OFFSET, container << 8);
    integer(&mut physical, ROOT_OFFSET, root);
    physical[FLAGS_OFFSET] = ORDINARY_FLAGS;
    active.extend(physical);
    active.extend_from_slice(&raw[layout.logical.clone()]);
    let mut logical = [0; LOGICAL_BYTES];
    integer(&mut logical, 0, LOGICAL_MAGIC);
    integer(&mut logical, LOGICAL_NUMBER_OFFSET, layout.logical_count);
    integer(&mut logical, PHYSICAL_NUMBER_OFFSET, layout.physical_count);
    integer(&mut logical, RELATED_INDEX_OFFSET, u32::MAX);
    active.extend(logical);
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
