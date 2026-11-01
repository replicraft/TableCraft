//! Independent original-row capacity replay and contiguous represented EOF claims.
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert, TableSchema,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        definition,
        layout::{RECORD_COUNT_OFFSET, TABLE_FREE_MAP_OFFSET},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::mutation::{
        normal_insert::proof::{Capacity, capacities, encode},
        record_insertion::properties,
        record_placement::future_pages::FuturePages,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const DIRECTORY_AT: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();
const COUNT_AT: usize = 12;
const FREE_AT: usize = 2;
const OWNER_AT: usize = 4;
const MAX_SLOTS: usize = 255;
const EMPTY_FREE: usize = PAGE_BYTES - DIRECTORY_AT;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let before_length = plan.before_bytes().len();
    if plan.after_bytes().len() < before_length
        || !plan.after_bytes().len().is_multiple_of(PAGE_BYTES)
        || plan.changed_rows() != requests.len()
    {
        return Err(EditError::Preservation);
    }
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::super::capture(&image, table, budget, cancelled)?;
    let schema = source.table.schema();
    properties::require_text_declarations(&image, schema, requests, budget, cancelled)?;
    super::super::records::qualify_complete(schema, requests, budget, cancelled)?;
    let encoded = encode(&image, &source, requests, budget, cancelled)?;
    let (mut pages, space) = capacities(&image, &source, budget, cancelled)?;
    budget
        .charge_allocation(before_length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, before_length as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(before_length)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend_from_slice(image.as_bytes());
    let first = image.catalog().header().page_count();
    let mut added = 0usize;
    for record in &encoded {
        check(budget, cancelled)?;
        let needed = record
            .len()
            .checked_add(SLOT_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let mut selected = None;
        for (position, page) in pages.iter().enumerate() {
            check(budget, cancelled)?;
            if page.eligible && page.count < MAX_SLOTS && needed <= page.free {
                selected = Some(position);
                break;
            }
        }
        let position = if let Some(position) = selected {
            position
        } else {
            if needed > EMPTY_FREE {
                return Err(EditError::Preservation);
            }
            let number = first
                .checked_add(u32::try_from(added).map_err(|_| InspectError::StructureLimit)?)
                .filter(|number| *number <= 0x00ff_ffff)
                .ok_or(InspectError::StructureLimit)?;
            let length = expected
                .len()
                .checked_add(PAGE_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            if u64::try_from(length).map_err(|_| InspectError::StructureLimit)? > MAX_IMAGE_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
            budget
                .charge_allocation(PAGE_BYTES as u64, 1)
                .map_err(InspectError::from)?;
            expected
                .try_reserve_exact(PAGE_BYTES)
                .map_err(|_| InspectError::Allocation)?;
            budget
                .charge(Resource::Steps, PAGE_BYTES as u64)
                .map_err(InspectError::from)?;
            check(budget, cancelled)?;
            let base = expected.len();
            expected.resize(length, 0);
            expected[base..base + 2].copy_from_slice(&[1, 1]);
            expected[base + FREE_AT..base + FREE_AT + SLOT_BYTES]
                .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
            expected[base + OWNER_AT..base + OWNER_AT + size_of::<u32>()]
                .copy_from_slice(&schema.definition_page().to_le_bytes());
            budget
                .charge_allocation(1, size_of::<Capacity>() as u64)
                .map_err(InspectError::from)?;
            budget
                .charge(
                    Resource::Steps,
                    pages
                        .len()
                        .checked_mul(size_of::<Capacity>())
                        .ok_or(InspectError::StructureLimit)? as u64,
                )
                .map_err(InspectError::from)?;
            pages
                .try_reserve_exact(1)
                .map_err(|_| InspectError::Allocation)?;
            pages.push(Capacity {
                number,
                count: 0,
                end: PAGE_BYTES,
                free: EMPTY_FREE,
                eligible: true,
            });
            added = added.checked_add(1).ok_or(InspectError::StructureLimit)?;
            pages.len() - 1
        };
        let page = &mut pages[position];
        if page.number < first {
            space.require_page(&image, page.number, false, budget, cancelled)?;
        }
        let start = page
            .end
            .checked_sub(record.len())
            .ok_or(EditError::Preservation)?;
        let count = page.count + 1;
        let free = start
            .checked_sub(DIRECTORY_AT + count * SLOT_BYTES)
            .ok_or(EditError::Preservation)?;
        let base = page.number as usize * PAGE_BYTES;
        let output = expected
            .get_mut(base..base + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if usize::from(u16_at(output, COUNT_AT)?) != page.count
            || usize::from(u16_at(output, FREE_AT)?) != page.free
        {
            return Err(EditError::Preservation);
        }
        budget
            .charge(Resource::Steps, (record.len() + 3 * SLOT_BYTES) as u64)
            .map_err(InspectError::from)?;
        check(budget, cancelled)?;
        output[start..page.end].copy_from_slice(record);
        let at = DIRECTORY_AT + page.count * SLOT_BYTES;
        output[at..at + SLOT_BYTES].copy_from_slice(
            &u16::try_from(start)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
        output[COUNT_AT..COUNT_AT + SLOT_BYTES].copy_from_slice(
            &u16::try_from(count)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
        output[FREE_AT..FREE_AT + SLOT_BYTES].copy_from_slice(
            &u16::try_from(free)
                .map_err(|_| EditError::Preservation)?
                .to_le_bytes(),
        );
        page.count = count;
        page.end = start;
        page.free = free;
    }
    if expected.len() != plan.after_bytes().len() {
        return Err(EditError::Preservation);
    }
    if added != 0 {
        let future = FuturePages::capture(&image, schema, added, budget, cancelled)?;
        future.require(&image, schema.definition_page())?;
    }
    space.audit(
        image.as_bytes(),
        &expected[..before_length],
        budget,
        cancelled,
    )?;
    maps(
        &image,
        schema,
        first,
        added,
        &mut expected,
        budget,
        cancelled,
    )?;
    let count_at = schema.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let original = u32_at(image.as_bytes(), count_at)?;
    let final_count = source
        .records
        .len()
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    if original != schema.declared_rows() || original as usize != source.records.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    expected[count_at..count_at + size_of::<u32>()].copy_from_slice(
        &u32::try_from(final_count)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    super::super::compare(&expected, plan.after_bytes(), budget, cancelled)?;
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::super::capture(&after, table, budget, cancelled)?;
    if final_source.table.schema().object_id() != schema.object_id()
        || final_source.table.schema().definition_page() != schema.definition_page()
        || final_source.records.len() != final_count
    {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn maps(
    image: &NativeImage,
    schema: &TableSchema,
    first: u32,
    count: usize,
    expected: &mut [u8],
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
    let raw = definition::read(&mut reader, schema.definition_page())?;
    let references = [
        GLOBAL_FREE_REFERENCE,
        schema.layout.usage,
        u32_at(&raw, TABLE_FREE_MAP_OFFSET)?,
    ];
    let roles = [MapRole::GlobalFree, MapRole::Owned, MapRole::TableFreeSpace];
    for (position, (reference, role)) in references.into_iter().zip(roles).enumerate() {
        reader.tick()?;
        let map = RepresentedMap::read(&mut reader, reference, role)?;
        for offset in 0..count {
            reader.tick()?;
            let number = first
                .checked_add(u32::try_from(offset).map_err(|_| InspectError::StructureLimit)?)
                .ok_or(InspectError::StructureLimit)?;
            let cell = map.future_cell(&mut reader, number)?;
            let actual = *expected.get(cell.offset).ok_or(EditError::Preservation)?;
            if cell.mask.count_ones() != 1
                || (cell.before & cell.mask != 0) != (position == 0)
                || image.as_bytes().get(cell.offset) != Some(&cell.before)
                || actual & cell.mask != cell.before & cell.mask
            {
                return Err(EditError::Preservation);
            }
            expected[cell.offset] = actual ^ cell.mask;
        }
    }
    Ok(())
}
