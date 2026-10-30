//! Original caller request and original records certify every candidate byte.
mod frame;
#[cfg(test)]
mod tests;

use super::{SecondaryIndexCreate, buffer, source, stage::Plan};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        allocation::{
            self, GLOBAL_FREE_REFERENCE, INLINE_BITMAP_OFFSET, INLINE_MAP, MapRole,
            QualifiedInlineRecord, RepresentedMap,
        },
        bytes::u32_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::reference,
    },
    header::check,
    table::{
        mutation::{
            append::owners::{MultipleOwners, OwnerContext},
            gates,
        },
        page::{self, TableSnapshotLimits},
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

// Independent wire law: these constants describe the original source profile,
// rather than consuming the producer's prepared keys or encoded output.
const ASCENDING_NULL_MARKER: u8 = 0;
const ASCENDING_VALUE_MARKER: u8 = 0x7f;
const INT32_SIGN_BIAS: u32 = 1 << (u32::BITS - 1);
const KEY_ROW_BYTES: usize = size_of::<u8>() + size_of::<i32>() + size_of::<u32>();
const CHILD_PAGE_BYTES: usize = size_of::<u32>();
const INLINE_BASE_OFFSET: usize = size_of::<u8>();

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    request: &SecondaryIndexCreate<'_>,
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let original = source::qualify(image, table, request, budget, cancelled)?;
    let columns = original
        .checked
        .schema()
        .scalar_columns_with_budget(budget)?;
    let records = page::read_records_batched(
        &mut image.reader(),
        original.checked.schema(),
        &columns,
        None,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )?;
    let mut keys = buffer::vector(records.len(), budget, cancelled)?;
    for (row, values) in &records {
        check(budget, cancelled)?;
        let value = match values.get(original.column) {
            Some(Value::Null) => None,
            Some(Value::Int32(value)) => Some(*value),
            _ => return Err(EditError::Preservation),
        };
        keys.push((value, reference(row.page_number(), row.slot_number())));
    }
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(KEY_ROW_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    keys.sort_unstable_by(|left, right| {
        let order = left.0.cmp(&right.0);
        let order = if request.descending {
            order.reverse()
        } else {
            order
        };
        order.then_with(|| left.1.cmp(&right.1))
    });
    let mut families = 0_u32;
    let mut previous = None;
    let mut wires = buffer::vector(keys.len(), budget, cancelled)?;
    for &(value, row) in &keys {
        check(budget, cancelled)?;
        if previous != Some(value) {
            families = families
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
            previous = Some(value);
        }
        let mut wire = buffer::vector(KEY_ROW_BYTES, budget, cancelled)?;
        if let Some(value) = value {
            wire.push(ASCENDING_VALUE_MARKER);
            wire.extend(((value as u32) ^ INT32_SIGN_BIAS).to_be_bytes());
        } else {
            wire.push(ASCENDING_NULL_MARKER);
        }
        if request.descending {
            for byte in &mut wire {
                *byte = !*byte;
            }
        }
        wire.extend(row.to_be_bytes());
        wires.push(wire);
    }
    let middle = if frame::fits(&wires, budget, cancelled)? {
        None
    } else {
        let middle = wires.len() / 2;
        if middle == 0
            || !frame::fits(&wires[..middle], budget, cancelled)?
            || !frame::fits(&wires[middle..], budget, cancelled)?
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Some(middle)
    };
    let bodies = if middle.is_some() { 3 } else { 1 };
    let container = u32::try_from(image.as_bytes().len() / PAGE_BYTES)
        .map_err(|_| InspectError::StructureLimit)?;
    let root = container
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    let end = root
        .checked_add(bodies)
        .ok_or(InspectError::StructureLimit)?;
    if end as usize > QualifiedInlineRecord::PAGE_CAPACITY
        || plan.append.original_length != image.as_bytes().len()
        || plan.append.length != end as usize * PAGE_BYTES
        || after.len() != plan.append.length
        || plan.indexed_rows != keys.len()
        || plan.distinct != families
        || plan.append.reservations_len() != bodies as usize + 1
    {
        return Err(EditError::Preservation);
    }
    let owner = &plan.append.owner;
    owner.census(image)?;
    if owner.definition() != original.layout.definition
        || owner.physical() != original.layout.physical_count
        || owner.logical() != original.layout.logical_count
        || owner.container() != container
        || owner.root() != root
        || owner.usage() != container << 8
        || owner.bodies() != bodies as usize
    {
        return Err(EditError::Preservation);
    }
    let mut maps = plan.append.created_index_maps();
    let map = maps.next().ok_or(EditError::Preservation)?;
    if maps.next().is_some()
        || map.number != container
        || map.definition != original.layout.definition
        || map.physical != original.layout.physical_count
    {
        return Err(EditError::Preservation);
    }
    let mut reservations = plan.append.created_indexes();
    for number in root..end {
        check(budget, cancelled)?;
        let reservation = reservations.next().ok_or(EditError::Preservation)?;
        if reservation.number != number
            || reservation.definition != original.layout.definition
            || reservation.physical != original.layout.physical_count
            || reservation.usage != container << 8
        {
            return Err(EditError::Preservation);
        }
    }
    if reservations.next().is_some() {
        return Err(EditError::Preservation);
    }

    let mut expected = buffer::vector(after.len(), budget, cancelled)?;
    for chunk in image
        .as_bytes()
        .chunks(crate::preservation::COPY_CHUNK_BYTES)
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        expected.extend_from_slice(chunk);
    }
    budget
        .charge(Resource::Steps, (after.len() - expected.len()) as u64)
        .map_err(InspectError::from)?;
    expected.resize(after.len(), 0);
    let definition = frame::definition(
        image, &original, root, container, families, budget, cancelled,
    )?;
    if plan.definition != definition {
        return Err(EditError::Preservation);
    }
    let definition_start = original.layout.definition as usize * PAGE_BYTES;
    expected[definition_start..definition_start + PAGE_BYTES].copy_from_slice(&definition);
    let mut pages = buffer::vector(bodies as usize + 1, budget, cancelled)?;
    pages.push(frame::map_page(root, bodies as usize, budget, cancelled)?);
    if let Some(middle) = middle {
        let left = root.checked_add(1).ok_or(InspectError::StructureLimit)?;
        let right = left.checked_add(1).ok_or(InspectError::StructureLimit)?;
        let mut bound = buffer::copy(&wires[middle - 1], budget, cancelled)?;
        budget
            .charge_allocation(CHILD_PAGE_BYTES as u64, size_of::<u8>() as u64)
            .map_err(InspectError::from)?;
        bound
            .try_reserve(CHILD_PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        bound.extend(left.to_be_bytes());
        pages.push(frame::index_page(
            original.layout.definition,
            (0, 0),
            right,
            1,
            &[bound],
            budget,
            cancelled,
        )?);
        pages.push(frame::index_page(
            original.layout.definition,
            (0, right),
            0,
            0,
            &wires[..middle],
            budget,
            cancelled,
        )?);
        pages.push(frame::index_page(
            original.layout.definition,
            (left, 0),
            0,
            0,
            &wires[middle..],
            budget,
            cancelled,
        )?);
    } else {
        pages.push(frame::index_page(
            original.layout.definition,
            (0, 0),
            0,
            0,
            &wires,
            budget,
            cancelled,
        )?);
    }
    if plan.pages.len() != pages.len() {
        return Err(EditError::Preservation);
    }
    for (position, bytes) in pages.iter().enumerate() {
        check(budget, cancelled)?;
        let number = container + position as u32;
        if plan.pages[position].number != number || plan.pages[position].bytes != *bytes {
            return Err(EditError::Preservation);
        }
        let start = number as usize * PAGE_BYTES;
        expected[start..start + PAGE_BYTES].copy_from_slice(bytes);
    }

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let storage =
        RepresentedMap::read_storage(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let raw_global = image
        .as_bytes()
        .get(storage.record.clone())
        .ok_or(InspectError::Truncated)?;
    let map_bytes = INLINE_BITMAP_OFFSET + QualifiedInlineRecord::PAGE_CAPACITY / u8::BITS as usize;
    if global.kind() != INLINE_MAP
        || raw_global.len() != map_bytes
        || u32_at(raw_global, INLINE_BASE_OFFSET)? != 0
        || owner.global_record() != storage.record
    {
        return Err(EditError::Preservation);
    }
    for number in container..end {
        let cell = global.future_cell(&mut reader, number)?;
        if cell.before & cell.mask == 0 {
            return Err(EditError::Preservation);
        }
        expected[cell.offset] &= !cell.mask;
    }
    let mut edits = plan.append.map_edits.iter();
    for offset in storage.record {
        reader.tick()?;
        let before = image.as_bytes()[offset];
        let after = expected[offset];
        if before != after {
            let edit = edits.next().ok_or(EditError::Preservation)?;
            if (edit.offset, edit.before, edit.after) != (offset, before, after) {
                return Err(EditError::Preservation);
            }
        }
    }
    if edits.next().is_some() {
        return Err(EditError::Preservation);
    }
    reader.work(after.len())?;
    if expected != after {
        return Err(EditError::Preservation);
    }

    let final_image = NativeImage::capture(
        &mut Cursor::new(after),
        image.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let final_checked = gates::before(&final_image, table, budget, cancelled)?;
    if final_checked.rows() != original.checked.rows()
        || final_checked.indexes().len() != original.checked.indexes().len() + 1
    {
        return Err(EditError::Preservation);
    }
    require_census(image, &original.checked, budget, cancelled)?;
    require_census(&final_image, &final_checked, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(keys.len())
}

fn require_census(
    image: &NativeImage,
    checked: &TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut pages = Vec::new();
    for index in checked.indexes() {
        for page in allocation::pages(&mut reader, index.index().usage_map())? {
            reader.tick()?;
            reader.work(pages.len())?;
            if !pages.contains(&page) {
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
                pages.push(page);
            }
        }
    }
    MultipleOwners::capture(image, &[checked], &pages, reader.budget, cancelled)?;
    Ok(())
}
