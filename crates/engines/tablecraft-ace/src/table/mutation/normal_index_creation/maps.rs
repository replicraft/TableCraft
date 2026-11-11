//! Fresh represented usage-map storage owns only the new INDEX population.
use crate::{
    EditError, InspectError,
    format::page::{DATA_SIGNATURE, PAGE_BYTES},
    header::check,
    table::{indexes::mutation::bulk_long::CreatedPage, mutation::index_creation::buffer},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DIRECTORY_END: usize = 16;
const INLINE_HEADER: usize = 1 + size_of::<u32>();
const INITIAL_BITMAP_BYTES: usize = 64;
const USAGE_HEADER: usize = 4;
const WINDOW: u32 = ((PAGE_BYTES - USAGE_HEADER) * u8::BITS as usize) as u32;

pub(super) struct Maps {
    pub pages: Vec<CreatedPage>,
    pub usage: u32,
    pub after_pages: u32,
}

pub(super) fn build(
    first: u32,
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Maps, EditError> {
    check(budget, cancelled)?;
    let count = u32::try_from(count).map_err(|_| InspectError::StructureLimit)?;
    if count == 0 {
        return Err(EditError::Preservation);
    }
    let host = first
        .checked_add(count)
        .ok_or(InspectError::StructureLimit)?;
    let usage = host
        .checked_mul(1 << u8::BITS)
        .ok_or(InspectError::StructureLimit)?;
    let bytes = usize::try_from(count)
        .map_err(|_| InspectError::StructureLimit)?
        .div_ceil(u8::BITS as usize)
        .max(INITIAL_BITMAP_BYTES);
    if INLINE_HEADER
        .checked_add(bytes)
        .is_some_and(|length| length <= PAGE_BYTES - DIRECTORY_END)
    {
        let mut payload = zero(INLINE_HEADER + bytes, budget, cancelled)?;
        payload[1..INLINE_HEADER].copy_from_slice(&first.to_le_bytes());
        for position in 0..count {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let position = position as usize;
            payload[INLINE_HEADER + position / u8::BITS as usize] |=
                1 << (position % u8::BITS as usize);
        }
        let mut pages = buffer::vector(1, budget, cancelled)?;
        pages.push(CreatedPage {
            number: host,
            bytes: container(&payload, budget, cancelled)?,
        });
        return Ok(Maps {
            pages,
            usage,
            after_pages: host.checked_add(1).ok_or(InspectError::StructureLimit)?,
        });
    }

    let last = host - 1;
    let start_window = first / WINDOW;
    let last_window = last / WINDOW;
    let windows = last_window
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    let pointer_bytes = usize::try_from(windows)
        .map_err(|_| InspectError::StructureLimit)?
        .checked_mul(size_of::<u32>())
        .ok_or(InspectError::StructureLimit)?;
    let row_bytes = 1_usize
        .checked_add(pointer_bytes)
        .ok_or(InspectError::StructureLimit)?;
    if row_bytes > PAGE_BYTES - DIRECTORY_END {
        return Err(InspectError::StructureLimit.into());
    }
    let active = last_window - start_window + 1;
    let after_pages = host
        .checked_add(1)
        .and_then(|page| page.checked_add(active))
        .ok_or(InspectError::StructureLimit)?;
    let mut payload = zero(row_bytes, budget, cancelled)?;
    payload[0] = 1;
    let mut pages = buffer::vector(active as usize + 1, budget, cancelled)?;
    // Container is emitted first; bitmap page references already have exact EOF roles.
    for window in start_window..=last_window {
        check(budget, cancelled)?;
        let number = host
            .checked_add(1)
            .and_then(|page| page.checked_add(window - start_window))
            .ok_or(InspectError::StructureLimit)?;
        let at = 1 + window as usize * size_of::<u32>();
        payload[at..at + size_of::<u32>()].copy_from_slice(&number.to_le_bytes());
    }
    pages.push(CreatedPage {
        number: host,
        bytes: container(&payload, budget, cancelled)?,
    });
    for window in start_window..=last_window {
        check(budget, cancelled)?;
        let mut raw = zero(PAGE_BYTES, budget, cancelled)?;
        raw[..USAGE_HEADER].copy_from_slice(&[5, 1, 0, 0]);
        let number = host
            .checked_add(1)
            .and_then(|page| page.checked_add(window - start_window))
            .ok_or(InspectError::StructureLimit)?;
        pages.push(CreatedPage { number, bytes: raw });
    }
    for page in first..host {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        let window = page / WINDOW;
        let offset = (page % WINDOW) as usize;
        let raw = &mut pages[(window - start_window) as usize + 1].bytes;
        raw[USAGE_HEADER + offset / u8::BITS as usize] |= 1 << (offset % u8::BITS as usize);
    }
    Ok(Maps {
        pages,
        usage,
        after_pages,
    })
}

fn container(
    payload: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let start = PAGE_BYTES
        .checked_sub(payload.len())
        .filter(|value| *value >= DIRECTORY_END)
        .ok_or(InspectError::StructureLimit)?;
    let mut raw = zero(PAGE_BYTES, budget, cancelled)?;
    raw[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
    raw[2..4].copy_from_slice(
        &u16::try_from(start - DIRECTORY_END)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    raw[12..14].copy_from_slice(&1_u16.to_le_bytes());
    raw[14..16].copy_from_slice(
        &u16::try_from(start)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    budget.charge(Resource::Steps, payload.len() as u64)?;
    raw[start..].copy_from_slice(payload);
    Ok(raw)
}

fn zero(
    length: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let mut bytes = buffer::vector(length, budget, cancelled)?;
    budget.charge(Resource::Steps, length as u64)?;
    bytes.resize(length, 0);
    Ok(bytes)
}
