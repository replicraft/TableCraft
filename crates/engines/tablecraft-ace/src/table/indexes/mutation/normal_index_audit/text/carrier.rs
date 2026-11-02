//! Independent exact-unit binding of original native Text storage cells.
use crate::{EditError, InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MARKER: [u8; 2] = [0xff, 0xfe];
const UNIT_BYTES: usize = size_of::<u16>();
const MODE_SWITCH: u8 = 0;

pub(super) fn require_payload(
    payload: &[u8],
    observed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // Keep the existing plain-cell pass and its charges/polls. A stored marker
    // selects native compressed framing even when compression preference is off.
    if !payload.starts_with(&MARKER) {
        return require_plain(payload, observed, budget, cancelled);
    }

    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, MARKER.len() as u64)
        .map_err(InspectError::from)?;
    let mut position = MARKER.len();
    let mut narrow = true;
    let mut units = observed.encode_utf16();
    while position < payload.len() {
        check(budget, cancelled)?;
        let first = payload[position];
        if first == MODE_SWITCH {
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            narrow = !narrow;
            position += 1;
            continue;
        }
        let (unit, width) = if narrow {
            (u16::from(first), 1)
        } else {
            let pair = payload
                .get(position..position + UNIT_BYTES)
                .ok_or(EditError::Preservation)?;
            (u16::from_le_bytes([pair[0], pair[1]]), UNIT_BYTES)
        };
        budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        if units.next() != Some(unit) {
            return Err(EditError::Preservation);
        }
        position += width;
    }
    check(budget, cancelled)?;
    if units.next().is_some() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn require_plain(
    payload: &[u8],
    observed: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if !payload.len().is_multiple_of(UNIT_BYTES) {
        return Err(EditError::Preservation);
    }
    let mut units = observed.encode_utf16();
    for bytes in payload.as_chunks::<UNIT_BYTES>().0 {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, UNIT_BYTES as u64)
            .map_err(InspectError::from)?;
        if units.next() != Some(u16::from_le_bytes(*bytes)) {
            return Err(EditError::Preservation);
        }
    }
    if units.next().is_some() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
