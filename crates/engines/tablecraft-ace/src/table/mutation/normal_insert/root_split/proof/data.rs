//! Independent one-record DATA exhaustion, inline bytes and map/count replay.
use crate::{
    EditError, InspectError, NativeImage, RowInsert,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::{RECORD_COUNT_OFFSET, TABLE_FREE_MAP_OFFSET},
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, SLOT_BYTES},
    },
    header::check,
    table::mutation::{
        fixed_records,
        normal_insert::proof::{capacities, encode},
        preserved_source::Source,
        record_placement::future::Future,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DIRECTORY_AT: usize = 14;
const COUNT_AT: usize = 12;
const FREE_AT: usize = 2;
const OWNER_AT: usize = 4;

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    if requests.len() != 1 {
        return Err(EditError::Preservation);
    }
    let encoded = encode(image, source, requests, budget, cancelled)?;
    let [record] = encoded.as_slice() else {
        return Err(EditError::Preservation);
    };
    let needed = record
        .len()
        .checked_add(SLOT_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let (pages, space) = capacities(image, source, budget, cancelled)?;
    for page in &pages {
        check(budget, cancelled)?;
        if page.eligible && page.count < MAX_SLOTS && needed <= page.free {
            return Err(EditError::Preservation);
        }
    }
    if record.is_empty() || needed > PAGE_BYTES - DIRECTORY_AT {
        return Err(EditError::Preservation);
    }
    let schema = source.table.schema();
    let future = Future::capture(image, schema, budget, cancelled)?;
    future.require(image, schema.definition_page())?;
    let mut expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    expected
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge(Resource::Steps, (PAGE_BYTES + record.len()) as u64)
        .map_err(InspectError::from)?;
    let base = expected.len();
    expected.resize(base + PAGE_BYTES, 0);
    let start = PAGE_BYTES
        .checked_sub(record.len())
        .ok_or(EditError::Preservation)?;
    let free = start
        .checked_sub(DIRECTORY_AT + SLOT_BYTES)
        .ok_or(EditError::Preservation)?;
    expected[base..base + 2].copy_from_slice(&[1, 1]);
    expected[base + OWNER_AT..base + OWNER_AT + size_of::<u32>()]
        .copy_from_slice(&schema.definition_page().to_le_bytes());
    expected[base + FREE_AT..base + FREE_AT + SLOT_BYTES].copy_from_slice(
        &u16::try_from(free)
            .map_err(|_| EditError::Preservation)?
            .to_le_bytes(),
    );
    expected[base + COUNT_AT..base + COUNT_AT + SLOT_BYTES].copy_from_slice(&1_u16.to_le_bytes());
    expected[base + DIRECTORY_AT..base + DIRECTORY_AT + SLOT_BYTES].copy_from_slice(
        &u16::try_from(start)
            .map_err(|_| EditError::Preservation)?
            .to_le_bytes(),
    );
    expected[base + start..base + PAGE_BYTES].copy_from_slice(record);
    space.audit(image.as_bytes(), &expected[..base], budget, cancelled)?;

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let definition = definition::read(&mut reader, schema.definition_page())?;
    let hint_ref = u32_at(&definition, TABLE_FREE_MAP_OFFSET)?;
    let hints = allocation::pages(&mut reader, hint_ref)?;
    let [hint] = hints.as_slice() else {
        return Err(EditError::Preservation);
    };
    let mut matched = false;
    for page in &pages {
        reader.tick()?;
        if page.number == *hint {
            if !page.eligible || (page.count < MAX_SLOTS && needed <= page.free) {
                return Err(EditError::Preservation);
            }
            matched = true;
        }
    }
    if !matched {
        return Err(EditError::Preservation);
    }
    for (position, (reference, role)) in [
        (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree),
        (schema.layout.usage, MapRole::Owned),
        (hint_ref, MapRole::TableFreeSpace),
    ]
    .into_iter()
    .enumerate()
    {
        reader.tick()?;
        let map = RepresentedMap::read(&mut reader, reference, role)?;
        let cell = map.future_cell(&mut reader, future.number())?;
        if cell.mask.count_ones() != 1
            || (cell.before & cell.mask != 0) != (position == 0)
            || expected.get(cell.offset) != Some(&cell.before)
        {
            return Err(EditError::Preservation);
        }
        expected[cell.offset] ^= cell.mask;
    }
    let hint_map = RepresentedMap::read(&mut reader, hint_ref, MapRole::TableFreeSpace)?;
    let old = hint_map.cell(&mut reader, *hint)?;
    if old.before & old.mask == 0 {
        return Err(EditError::Preservation);
    }
    expected[old.offset] &= !old.mask;
    let count_at = schema.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    if u32_at(image.as_bytes(), count_at)? as usize != source.records.len() {
        return Err(EditError::Preservation);
    }
    let total = u32::try_from(source.records.len())
        .map_err(|_| InspectError::StructureLimit)?
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    expected[count_at..count_at + size_of::<u32>()].copy_from_slice(&total.to_le_bytes());
    reader.work(3 * size_of::<u32>() + 3 * SLOT_BYTES)?;
    reader.tick()?;
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut references = Vec::new();
    references
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    references.push(future.number() << u8::BITS);
    Ok((expected, references))
}
