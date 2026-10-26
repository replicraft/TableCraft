//! Exact union proof for global bitmap bytes touched by several table owners.
use super::{GLOBAL_MAP_REFERENCE, INVALID_PREFIX_BYTES, RetirementTable};
use crate::{
    EditError, InspectError,
    format::{
        allocation::{MapCell, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    inspect_header,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) fn audit_global(
    before: &[u8],
    after: &[u8],
    tables: &[RetirementTable<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let count = tables.iter().try_fold(0_usize, |count, table| {
        count
            .checked_add(table.retired.len())
            .ok_or(InspectError::StructureLimit)
    })?;
    if count == 0 {
        return Ok(());
    }
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<(u32, MapCell)>() as u64)
        .map_err(InspectError::from)?;
    let mut cells: Vec<(u32, MapCell)> = Vec::new();
    cells
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut source = Cursor::new(before);
    let header = inspect_header(&mut source, budget, cancelled)?;
    {
        let mut reader = Reader {
            source: &mut source,
            header,
            budget,
            cancelled,
            reads: 0,
        };
        let map = RepresentedMap::read(&mut reader, GLOBAL_MAP_REFERENCE, MapRole::GlobalFree)?;
        for table in tables {
            for page in table.retired {
                reader.tick()?;
                reader.work(cells.len())?;
                if cells.iter().any(|(number, _)| *number == page.number) {
                    return Err(EditError::Preservation);
                }
                let cell = map.cell(&mut reader, page.number)?;
                if cell.before & cell.mask != 0 {
                    return Err(EditError::Preservation);
                }
                cells.push((page.number, cell));
            }
        }
    }
    let mut source = Cursor::new(after);
    let header = inspect_header(&mut source, budget, cancelled)?;
    let mut reader = Reader {
        source: &mut source,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let map = RepresentedMap::read(&mut reader, GLOBAL_MAP_REFERENCE, MapRole::GlobalFree)?;
    for (number, original) in &cells {
        reader.tick()?;
        reader.work(cells.len())?;
        let mask = cells
            .iter()
            .filter(|(_, cell)| cell.offset == original.offset)
            .fold(0, |mask, (_, cell)| mask | cell.mask);
        let current = map.cell(&mut reader, *number)?;
        let offset = *number as usize * PAGE_BYTES;
        let old = before
            .get(offset..offset + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let new = after
            .get(offset..offset + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        reader.work(PAGE_BYTES)?;
        if current.offset != original.offset
            || current.before != (original.before | mask)
            || new[..INVALID_PREFIX_BYTES] != [0; INVALID_PREFIX_BYTES]
            || old[INVALID_PREFIX_BYTES..] != new[INVALID_PREFIX_BYTES..]
        {
            return Err(EditError::Preservation);
        }
    }
    check(reader.budget, reader.cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
