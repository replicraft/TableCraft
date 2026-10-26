use super::{GLOBAL_MAP_REFERENCE, INVALID_PREFIX_BYTES, RetiredPage};
use crate::format::{
    allocation::{self, MapCell, MapRole, RepresentedMap},
    page::PAGE_BYTES,
    reader::Reader,
};
use crate::{EditError, InspectError, inspect_header, inspect_table_indexes};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

struct Ownership {
    physical: u32,
    root: u32,
    usage: u32,
    pages: Vec<u32>,
}

pub(in crate::table) fn audit_retirements(
    before: &[u8],
    after: &[u8],
    scope: CatalogScope,
    table: &str,
    retired: &[RetiredPage],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if retired.is_empty() {
        return Ok(());
    }
    let mut old_source = Cursor::new(before);
    let mut new_source = Cursor::new(after);
    let old_indexes = inspect_table_indexes(&mut old_source, scope, table, budget, cancelled)?;
    let new_indexes = inspect_table_indexes(&mut new_source, scope, table, budget, cancelled)?;
    if old_indexes.indexes().len() != new_indexes.indexes().len() {
        return Err(EditError::Preservation);
    }
    let old_header = inspect_header(&mut old_source, budget, cancelled)?;
    let new_header = inspect_header(&mut new_source, budget, cancelled)?;
    let mut original_cells = Vec::new();
    let mut ownership: Vec<Ownership> = Vec::new();
    {
        let mut reader = Reader {
            source: &mut old_source,
            header: old_header,
            budget,
            cancelled,
            reads: 0,
        };
        let global = RepresentedMap::read(&mut reader, GLOBAL_MAP_REFERENCE, MapRole::GlobalFree)?;
        reader
            .budget
            .charge_allocation(retired.len() as u64, size_of::<MapCell>() as u64)
            .map_err(InspectError::from)?;
        original_cells
            .try_reserve_exact(retired.len())
            .map_err(|_| InspectError::Allocation)?;
        for page in retired {
            let cell = global.cell(&mut reader, page.number)?;
            if cell.before & cell.mask != 0 {
                return Err(EditError::Preservation);
            }
            original_cells.push(cell);
        }
        for original in old_indexes.indexes() {
            let current = new_indexes
                .indexes()
                .iter()
                .find(|index| index.number() == original.number())
                .ok_or(EditError::Preservation)?;
            reader.work(new_indexes.indexes().len() + ownership.len())?;
            if original.root_page() != current.root_page()
                || original.usage_map() != current.usage_map()
                || original.data_number() != current.data_number()
            {
                return Err(EditError::Preservation);
            }
            if ownership
                .iter()
                .any(|previous| previous.physical == original.data_number())
            {
                continue;
            }
            let pages = allocation::pages(&mut reader, original.usage_map())?;
            reader
                .budget
                .charge_allocation(1, size_of::<Ownership>() as u64)
                .map_err(InspectError::from)?;
            ownership
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            ownership.push(Ownership {
                physical: original.data_number(),
                root: original.root_page(),
                usage: original.usage_map(),
                pages,
            });
        }
    }
    let mut reader = Reader {
        source: &mut new_source,
        header: new_header,
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_MAP_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    global.require_allocated(&mut reader, new_indexes.schema().definition)?;
    let data_map = RepresentedMap::read(
        &mut reader,
        new_indexes.schema().layout.usage,
        MapRole::Owned,
    )?;
    data_map.require_allocated_storage(&mut reader, &global)?;
    for number in allocation::pages(&mut reader, new_indexes.schema().layout.usage)? {
        global.require_allocated(&mut reader, number)?;
    }

    for (page, original) in retired.iter().zip(&original_cells) {
        reader.work(original_cells.len())?;
        let mask = original_cells
            .iter()
            .filter(|cell| cell.offset == original.offset)
            .fold(0, |mask, cell| mask | cell.mask);
        let cell = global.cell(&mut reader, page.number)?;
        let start = page.number as usize * PAGE_BYTES;
        let old_page = before
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let new_page = after
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if cell.offset != original.offset
            || cell.before != (original.before | mask)
            || new_page[..INVALID_PREFIX_BYTES] != [0; INVALID_PREFIX_BYTES]
            || new_page[INVALID_PREFIX_BYTES..] != old_page[INVALID_PREFIX_BYTES..]
        {
            return Err(EditError::Preservation);
        }
    }
    for original in ownership {
        let map = RepresentedMap::read(&mut reader, original.usage, MapRole::Owned)?;
        map.require_allocated_storage(&mut reader, &global)?;
        let current = allocation::pages(&mut reader, original.usage)?;
        reader.work(original.pages.len() * (retired.len() + current.len()))?;
        let mut expected_count = 0;
        for number in original.pages {
            let removed = retired
                .iter()
                .any(|page| page.physical == original.physical && page.number == number);
            if removed {
                if current.binary_search(&number).is_ok() {
                    return Err(EditError::Preservation);
                }
            } else {
                expected_count += 1;
                if current.binary_search(&number).is_err() {
                    return Err(EditError::Preservation);
                }
                let cell = global.cell(&mut reader, number)?;
                if cell.before & cell.mask != 0 {
                    return Err(EditError::Preservation);
                }
            }
        }
        if (new_indexes.schema().declared_rows() == 0 && current.as_slice() != [original.root])
            || current.len() != expected_count
            || current.binary_search(&original.root).is_err()
        {
            return Err(EditError::Preservation);
        }
    }
    reader.tick()?;
    Ok(())
}
