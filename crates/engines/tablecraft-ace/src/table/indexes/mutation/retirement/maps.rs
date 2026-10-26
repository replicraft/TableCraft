use super::{
    FIRST_REUSABLE_PAGE, GLOBAL_MAP_REFERENCE, INVALID_PREFIX_BYTES, RetiredPage, append,
    header_patch,
};
use crate::format::{
    allocation::{self, MapCell, MapRole, RepresentedMap},
    bytes::u32_at,
    page::OWNER_OFFSET,
    reader::Reader,
};
use crate::table::indexes::{
    IndexCheck,
    mutation::IndexDataPatch,
    page::{LEAF, NODE},
};
use crate::{EditError, InspectError, NativeImage, TableSchema, header::check};
use tablecraft_core::limits::ResourceBudget;

struct ByteEdit {
    offset: usize,
    before: u8,
    set: u8,
    clear: u8,
}

fn merge(
    edits: &mut Vec<ByteEdit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(edits.len())?;
    let current = cell.before & cell.mask != 0;
    if current == set {
        return Err(InspectError::Malformed.into());
    }
    if let Some(edit) = edits.iter_mut().find(|edit| edit.offset == cell.offset) {
        if edit.before != cell.before || (edit.set | edit.clear) & cell.mask != 0 {
            return Err(EditError::Preservation);
        }
        if set {
            edit.set |= cell.mask;
        } else {
            edit.clear |= cell.mask;
        }
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<ByteEdit>() as u64)
            .map_err(InspectError::from)?;
        edits.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        edits.push(ByteEdit {
            offset: cell.offset,
            before: cell.before,
            set: if set { cell.mask } else { 0 },
            clear: if set { 0 } else { cell.mask },
        });
    }
    Ok(())
}

pub(in crate::table::indexes::mutation) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    retired: &[RetiredPage],
    patches: &mut Vec<IndexDataPatch>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_MAP_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    global.require_allocated(&mut reader, schema.definition)?;
    let data_map = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
    data_map.require_allocated_storage(&mut reader, &global)?;
    let data = allocation::pages(&mut reader, schema.layout.usage)?;
    for number in &data {
        global.require_allocated(&mut reader, *number)?;
    }
    let mut edits = Vec::new();
    // Root and every retained index page must be allocated. Distinct physical
    // declarations may not share owned children merely because roots differ.
    for (position, check) in indexes.iter().enumerate() {
        let index = check.index();
        if indexes[..position]
            .iter()
            .any(|previous| previous.index().data_number() == index.data_number())
        {
            continue;
        }
        let owned = allocation::pages(&mut reader, index.usage_map())?;
        for number in &owned {
            let free = global.cell(&mut reader, *number)?;
            if free.before & free.mask != 0 || data.binary_search(number).is_ok() {
                return Err(InspectError::Malformed.into());
            }
        }
        for other in &indexes[..position] {
            if other.index().data_number() == index.data_number() {
                continue;
            }
            let previous = allocation::pages(&mut reader, other.index().usage_map())?;
            reader.work(owned.len() * previous.len())?;
            if owned
                .iter()
                .any(|number| previous.binary_search(number).is_ok())
            {
                return Err(InspectError::Malformed.into());
            }
        }
    }
    for (position, page) in retired.iter().enumerate() {
        reader.work(position)?;
        if page.number == page.root
            || page.number < FIRST_REUSABLE_PAGE
            || retired[..position]
                .iter()
                .any(|previous| previous.number == page.number)
        {
            return Err(EditError::Preservation);
        }
        let declaration = indexes
            .iter()
            .find(|index| index.index().data_number() == page.physical)
            .ok_or(EditError::Preservation)?
            .index();
        if declaration.root_page() != page.root || declaration.usage_map() != page.usage {
            return Err(EditError::Preservation);
        }
        let raw = reader.page(page.number)?;
        if !matches!(raw[0], NODE | LEAF) || u32_at(&raw, OWNER_OFFSET)? != schema.definition {
            return Err(InspectError::Malformed.into());
        }
        let owned = RepresentedMap::read(&mut reader, page.usage, MapRole::Owned)?;
        owned.require_allocated_storage(&mut reader, &global)?;
        merge(
            &mut edits,
            owned.cell(&mut reader, page.number)?,
            false,
            &mut reader,
        )?;
        merge(
            &mut edits,
            global.cell(&mut reader, page.number)?,
            true,
            &mut reader,
        )?;
        header_patch(
            image,
            page.number,
            0,
            &[0; INVALID_PREFIX_BYTES],
            patches,
            reader.budget,
            reader.cancelled,
        )?;
    }
    for edit in edits {
        let after = (edit.before | edit.set) & !edit.clear;
        append(
            patches,
            edit.offset,
            &[edit.before],
            &[after],
            reader.budget,
            reader.cancelled,
        )?;
    }
    check(reader.budget, reader.cancelled)?;
    Ok(())
}
