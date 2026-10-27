//! Derive exact role bytes from source descriptors and certified new pages.
use super::super::maps::Edit;
use crate::{
    EditError, InspectError,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        reader::Reader,
    },
};
use std::io::Cursor;

pub(super) struct Expected {
    offset: usize,
    before: u8,
    after: u8,
}

fn bit(
    expected: &mut Vec<Expected>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(expected.len())?;
    if let Some(prior) = expected
        .iter_mut()
        .find(|prior| prior.offset == cell.offset)
    {
        if prior.before != cell.before {
            return Err(EditError::Preservation);
        }
        prior.after = if set {
            prior.after | cell.mask
        } else {
            prior.after & !cell.mask
        };
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<Expected>() as u64)
            .map_err(InspectError::from)?;
        expected
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        expected.push(Expected {
            offset: cell.offset,
            before: cell.before,
            after: if set {
                cell.before | cell.mask
            } else {
                cell.before & !cell.mask
            },
        });
    }
    Ok(())
}

pub(super) fn add<const N: usize>(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    usage: u32,
    numbers: [u32; N],
    expected: &mut Vec<Expected>,
) -> Result<(), EditError> {
    let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(reader, &global)?;
    let owned = RepresentedMap::read(reader, usage, MapRole::Owned)?;
    owned.require_allocated_storage(reader, &global)?;
    for number in numbers {
        reader.tick()?;
        let own = owned.future_cell(reader, number)?;
        let free = global.future_cell(reader, number)?;
        if own.before & own.mask != 0 || free.before & free.mask == 0 {
            return Err(EditError::Preservation);
        }
        bit(expected, own, true, reader)?;
        bit(expected, free, false, reader)?;
    }
    Ok(())
}

pub(super) fn finish(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    expected: &[Expected],
    actual: &[Edit],
    after: &[u8],
) -> Result<(), EditError> {
    if expected.len() != actual.len() {
        return Err(EditError::Preservation);
    }
    for byte in expected {
        reader.tick()?;
        reader.work(actual.len())?;
        let matching = actual
            .iter()
            .find(|edit| edit.offset == byte.offset)
            .ok_or(EditError::Preservation)?;
        if matching.before != byte.before
            || matching.after != byte.after
            || after.get(byte.offset) != Some(&byte.after)
        {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}
