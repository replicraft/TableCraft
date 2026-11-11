//! Canonical wire framing from decoded bytes, not producer partitions/codecs.
use crate::{
    EditError, InspectError, WriteBlocker, header::check, table::mutation::index_creation::buffer,
};
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
const USAGE_WINDOW: u32 = (4096 - 4) * 8;

pub(super) struct Appended {
    pub first: u32,
    pub host: u32,
    pub root: u32,
    pub owner: u32,
    pub descending: bool,
}

pub(super) fn appended(
    bytes: &[u8],
    layout: Appended,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let Appended {
        first,
        host,
        root,
        owner,
        descending,
    } = layout;
    if root.checked_add(1) != Some(host) || host <= first || !bytes.len().is_multiple_of(PAGE) {
        return Err(EditError::Preservation);
    }
    let mut level_start = first;
    let mut previous_height = 0;
    for number in first..host {
        check(budget, cancelled)?;
        let raw = page(bytes, number)?;
        let height = raw[HEIGHT_OFFSET];
        if (number == first && height != 0)
            || height < previous_height
            || height
                > previous_height
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?
        {
            return Err(EditError::Preservation);
        }
        if height != previous_height {
            level_start = number;
        }
        let next_height = if number + 1 < host {
            Some(page(bytes, number + 1)?[HEIGHT_OFFSET])
        } else {
            None
        };
        let previous = if number == level_start { 0 } else { number - 1 };
        let next = if next_height == Some(height) {
            number + 1
        } else {
            0
        };
        let tail = integer_at(raw, TAIL_OFFSET)?;
        if integer_at(raw, OWNER_OFFSET)? != owner
            || integer_at(raw, PREVIOUS_OFFSET)? != previous
            || integer_at(raw, NEXT_OFFSET)? != next
            || (height == 0 && tail != 0)
        {
            return Err(EditError::Preservation);
        }
        let wires = entries(raw, height, descending, budget, cancelled)?;
        let expected = index_page(
            owner,
            (previous, next),
            tail,
            height,
            &wires,
            budget,
            cancelled,
        )?;
        budget
            .charge(Resource::Steps, PAGE as u64)
            .map_err(InspectError::from)?;
        if raw != expected.as_slice() {
            return Err(EditError::Preservation);
        }
        previous_height = height;
    }
    if level_start != root {
        return Err(EditError::Preservation);
    }
    maps(bytes, first, host, budget, cancelled)
}

fn entries(
    raw: &[u8],
    height: u8,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let active = CAPACITY
        .checked_sub(usize::from(word_at(raw, FREE_OFFSET)?))
        .ok_or(EditError::Preservation)?;
    let prefix = usize::from(word_at(raw, PREFIX_OFFSET)?);
    let mut output: Vec<Vec<u8>> = Vec::new();
    let mut begin = 0;
    for end in 1..=active {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        if raw[END_BITMAP_OFFSET + end / 8] & (1 << (end % 8)) == 0 {
            continue;
        }
        let suffix = &raw[START + begin..START + end];
        let shared = if output.is_empty() {
            &[][..]
        } else {
            output[0].get(..prefix).ok_or(EditError::Preservation)?
        };
        let length = shared
            .len()
            .checked_add(suffix.len())
            .ok_or(InspectError::StructureLimit)?;
        let mut wire = buffer::vector(length, budget, cancelled)?;
        wire.extend_from_slice(shared);
        wire.extend_from_slice(suffix);
        let marker = *wire.first().ok_or(EditError::Preservation)?;
        let marker = if descending { !marker } else { marker };
        let key_row = match marker {
            0 => 5,
            0x7f => 9,
            _ => return Err(EditError::Preservation),
        };
        if wire.len() != key_row + if height == 0 { 0 } else { 4 }
            || (!output.is_empty() && prefix > key_row)
        {
            return Err(EditError::Preservation);
        }
        buffer::push(&mut output, wire, budget, cancelled)?;
        begin = end;
    }
    if begin != active || (height > 0 && output.is_empty()) {
        return Err(EditError::Preservation);
    }
    Ok(output)
}

fn maps(
    bytes: &[u8],
    first: u32,
    host: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let count = host.checked_sub(first).ok_or(EditError::Preservation)?;
    let bitmap = (count as usize).div_ceil(8).max(64);
    let inline = bitmap.checked_add(5).ok_or(InspectError::StructureLimit)?;
    let mut payload;
    let expected_pages;
    if inline <= PAGE - 16 {
        payload = zero(inline, budget, cancelled)?;
        integer(&mut payload, 1, first);
        for bit in 0..count {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            payload[5 + bit as usize / 8] |= 1 << (bit % 8);
        }
        expected_pages = host.checked_add(1).ok_or(InspectError::StructureLimit)?;
    } else {
        let first_window = first / USAGE_WINDOW;
        let last_window = (host - 1) / USAGE_WINDOW;
        let slots = last_window
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?;
        let length = (slots as usize)
            .checked_mul(4)
            .and_then(|length| length.checked_add(1))
            .filter(|length| *length <= PAGE - 16)
            .ok_or(InspectError::StructureLimit)?;
        payload = zero(length, budget, cancelled)?;
        payload[0] = 1;
        expected_pages = host
            .checked_add(2)
            .and_then(|number| number.checked_add(last_window - first_window))
            .ok_or(InspectError::StructureLimit)?;
        for window in first_window..=last_window {
            check(budget, cancelled)?;
            let number = host
                .checked_add(1)
                .and_then(|number| number.checked_add(window - first_window))
                .ok_or(InspectError::StructureLimit)?;
            integer(&mut payload, 1 + window as usize * 4, number);
            let mut expected = zero(PAGE, budget, cancelled)?;
            expected[..4].copy_from_slice(&[5, 1, 0, 0]);
            let start = window
                .checked_mul(USAGE_WINDOW)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(USAGE_WINDOW)
                .ok_or(InspectError::StructureLimit)?
                .min(host);
            for member in start.max(first)..end {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                let bit = (member - start) as usize;
                expected[4 + bit / 8] |= 1 << (bit % 8);
            }
            if page(bytes, number)? != expected.as_slice() {
                return Err(EditError::Preservation);
            }
        }
    }
    if bytes.len() / PAGE != expected_pages as usize {
        return Err(EditError::Preservation);
    }
    let mut expected = zero(PAGE, budget, cancelled)?;
    expected[..2].copy_from_slice(&[1, 1]);
    let offset = PAGE
        .checked_sub(payload.len())
        .ok_or(EditError::Preservation)?;
    word(
        &mut expected,
        2,
        u16::try_from(offset - 16).map_err(|_| InspectError::StructureLimit)?,
    );
    word(&mut expected, 12, 1);
    word(
        &mut expected,
        14,
        u16::try_from(offset).map_err(|_| InspectError::StructureLimit)?,
    );
    expected[offset..].copy_from_slice(&payload);
    if page(bytes, host)? != expected.as_slice() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn zero(
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut output = buffer::vector(length, budget, cancelled)?;
    budget.charge(Resource::Steps, length as u64)?;
    output.resize(length, 0);
    Ok(output)
}

fn page(bytes: &[u8], number: u32) -> Result<&[u8], EditError> {
    let start = (number as usize)
        .checked_mul(PAGE)
        .ok_or(InspectError::StructureLimit)?;
    bytes
        .get(
            start
                ..start
                    .checked_add(PAGE)
                    .ok_or(InspectError::StructureLimit)?,
        )
        .ok_or(EditError::Preservation)
}
fn word_at(bytes: &[u8], offset: usize) -> Result<u16, EditError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    ))
}
fn integer_at(bytes: &[u8], offset: usize) -> Result<u32, EditError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
    ))
}
fn word(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn integer(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
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
