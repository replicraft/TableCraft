//! Fresh independent record encoding and original-capacity placement replay.
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowInsert,
    format::{
        allocation::{self, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        definition,
        layout::{RECORD_COUNT_OFFSET, TABLE_FREE_MAP_OFFSET},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::mutation::{
        fixed_records,
        preserved_source::Source,
        record_insertion::{proof as records, properties},
        record_placement::space::Space,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const SIGNATURE: [u8; 2] = [1, 1];
const OWNER_AT: usize = 4;
const FREE_AT: usize = 2;
const COUNT_AT: usize = 12;
const DIRECTORY_AT: usize = 14;
const DIRECTORY_BYTES: usize = size_of::<u16>();
const MAX_SLOTS: usize = 255;
const OFFSET_MASK: u16 = 0x1fff;
const DELETED_MASK: u16 = 0x8000;
const UNSUPPORTED_MASK: u16 = 0x2000;
const MAX_REFERENCE_PAGE: u32 = u32::MAX >> u8::BITS;

pub(super) struct Capacity {
    pub(super) number: u32,
    pub(super) count: usize,
    pub(super) end: usize,
    pub(super) free: usize,
    pub(super) eligible: bool,
}

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.ordinary.insert.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = super::capture(&image, table, budget, cancelled)?;
    source.require_preserved_definition_tail(&image, plan.after_bytes(), budget, cancelled)?;
    properties::require_text_declarations(
        &image,
        source.table.schema(),
        requests,
        budget,
        cancelled,
    )?;
    super::records::qualify(source.table.schema(), requests, budget, cancelled)?;

    if plan.before_bytes().len() != plan.after_bytes().len()
        || plan.changed_rows() != requests.len()
    {
        return Err(EditError::Preservation);
    }

    let expected = reconstruct(&image, &source, requests, budget, cancelled)?;
    super::compare(&expected, plan.after_bytes(), budget, cancelled)?;

    // Complete final Source validates all final rows/properties/LVAL and
    // dependencies; INSERT intentionally increases the original row count.
    let after = NativeImage::capture(&mut Cursor::new(&expected), scope, budget, cancelled)?;
    let final_source = super::capture(&after, table, budget, cancelled)?;
    let schema = source.table.schema();
    let total = source.records.len() + requests.len();
    if final_source.table.schema().object_id() != schema.object_id()
        || final_source.table.schema().definition_page() != schema.definition_page()
        || final_source.records.len() != total
        || final_source.table.schema().declared_rows() as usize != total
    {
        return Err(EditError::Preservation);
    }

    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    reconstruct_records(image, source, requests, false, budget, cancelled)
        .map(|(expected, _)| expected)
}

/// Return request-aligned ROW identities from this proof's own placement replay.
/// These references never come from producer reservations or the observed AFTER.
pub(super) fn reconstruct_with_references(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    reconstruct_records(image, source, requests, true, budget, cancelled)
}

fn reconstruct_records(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    retain_references: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    let encoded = encode(image, source, requests, budget, cancelled)?;
    reconstruct_encoded(
        image,
        source,
        requests,
        encoded,
        retain_references,
        budget,
        cancelled,
    )
}

pub(super) fn reconstruct_incrementing(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    let schema = source.table.schema();
    let shape = if schema.columns().len() != schema.layout.max_columns {
        records::layout::capture_sparse_incrementing(schema, column, budget, cancelled)?
    } else {
        records::layout::capture_incrementing(schema, column, budget, cancelled)?
    };
    let encoded = encode_with_shape(image, source, requests, shape, budget, cancelled)?;
    reconstruct_encoded(image, source, requests, encoded, true, budget, cancelled)
}

pub(in crate::table::mutation::normal_insert) fn reconstruct_encoded(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    encoded: Vec<Vec<u8>>,
    retain_references: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    reconstruct_encoded_with_limit(
        image,
        source,
        requests,
        encoded,
        Reconstruction {
            retain_references,
            maximum_rows: Some(crate::limits::MAX_LIVE_ROWS),
        },
        budget,
        cancelled,
    )
}

#[cfg(feature = "ace16-create")]
pub(in crate::table::mutation::normal_insert) fn reconstruct_retained_encoded(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    encoded: Vec<Vec<u8>>,
    retain_references: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    reconstruct_encoded_with_limit(
        image,
        source,
        requests,
        encoded,
        Reconstruction {
            retain_references,
            maximum_rows: None,
        },
        budget,
        cancelled,
    )
}

struct Reconstruction {
    retain_references: bool,
    maximum_rows: Option<usize>,
}

fn reconstruct_encoded_with_limit(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    encoded: Vec<Vec<u8>>,
    options: Reconstruction,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u32>), EditError> {
    let retain_references = options.retain_references;
    let (mut capacities, space) = capacities(image, source, budget, cancelled)?;
    let mut expected = fixed_records::copy(image.as_bytes(), budget, cancelled)?;

    let mut references = Vec::new();
    if retain_references {
        budget
            .charge_allocation(requests.len() as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        references
            .try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
    }

    for record in &encoded {
        check(budget, cancelled)?;
        let mut selected = None;
        for capacity in &mut capacities {
            check(budget, cancelled)?;
            let required = record
                .len()
                .checked_add(DIRECTORY_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            if !capacity.eligible || capacity.count == MAX_SLOTS || required > capacity.free {
                continue;
            }

            let start = capacity
                .end
                .checked_sub(record.len())
                .ok_or(EditError::Preservation)?;
            let next_count = capacity.count + 1;
            let next_free = start
                .checked_sub(DIRECTORY_AT + DIRECTORY_BYTES * next_count)
                .ok_or(EditError::Preservation)?;
            space.require_page(image, capacity.number, false, budget, cancelled)?;

            let base = capacity.number as usize * PAGE_BYTES;
            let page = expected
                .get_mut(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, (record.len() + 3 * DIRECTORY_BYTES) as u64)
                .map_err(InspectError::from)?;
            check(budget, cancelled)?;
            page[start..capacity.end].copy_from_slice(record);

            let slot = DIRECTORY_AT + capacity.count * DIRECTORY_BYTES;
            let word = u16::try_from(start).map_err(|_| InspectError::StructureLimit)?;
            page[slot..slot + DIRECTORY_BYTES].copy_from_slice(&word.to_le_bytes());
            let count = u16::try_from(next_count).map_err(|_| InspectError::StructureLimit)?;
            page[COUNT_AT..COUNT_AT + size_of::<u16>()].copy_from_slice(&count.to_le_bytes());
            let free = u16::try_from(next_free).map_err(|_| InspectError::StructureLimit)?;
            page[FREE_AT..FREE_AT + size_of::<u16>()].copy_from_slice(&free.to_le_bytes());

            // Record the destination at the moment this request claims it.
            // Later smaller rows can legitimately fit an earlier owned page.
            if retain_references {
                if capacity.number > MAX_REFERENCE_PAGE || capacity.count >= MAX_SLOTS {
                    return Err(EditError::Preservation);
                }
                let slot =
                    u32::try_from(capacity.count).map_err(|_| InspectError::StructureLimit)?;
                budget
                    .charge(Resource::Steps, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                references.push((capacity.number << u8::BITS) | slot);
            }

            capacity.end = start;
            capacity.count = next_count;
            capacity.free = next_free;
            selected = Some(());
            break;
        }
        if selected.is_none() {
            return Err(EditError::Preservation);
        }
    }

    space.audit(image.as_bytes(), &expected, budget, cancelled)?;
    let schema = source.table.schema();
    let count_at = schema.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let count = u32_at(image.as_bytes(), count_at)?;
    let added = u32::try_from(requests.len()).map_err(|_| InspectError::StructureLimit)?;
    let total = count
        .checked_add(added)
        .ok_or(InspectError::StructureLimit)?;
    if count != schema.declared_rows()
        || options.maximum_rows.is_some_and(|maximum| {
            usize::try_from(total)
                .ok()
                .is_none_or(|value| value > maximum)
        })
    {
        return Err(EditError::Preservation);
    }

    budget
        .charge(Resource::Steps, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    expected[count_at..count_at + size_of::<u32>()].copy_from_slice(&total.to_le_bytes());
    Ok((expected, references))
}

pub(super) fn encode(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let schema = source.table.schema();
    let shape = records::layout::capture_actions(schema, budget, cancelled)?;
    encode_with_shape(image, source, requests, shape, budget, cancelled)
}

fn encode_with_shape(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
    shape: records::layout::Shape,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let schema = source.table.schema();
    budget
        .charge_allocation(requests.len() as u64, size_of::<Vec<u8>>() as u64)
        .map_err(InspectError::from)?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for request in requests {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        reader
            .budget
            .charge_allocation(shape.variables.len() as u64, size_of::<Vec<u8>>() as u64)
            .map_err(InspectError::from)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(shape.variables.len())
            .map_err(|_| InspectError::Allocation)?;

        let mut end = shape.fixed_end;
        for position in &shape.variables {
            reader.tick()?;
            let payload = records::payload::encode(
                &schema.columns()[*position],
                &request.values()[*position],
                reader.budget,
                reader.cancelled,
            )?;
            end = end
                .checked_add(payload.len())
                .ok_or(InspectError::StructureLimit)?;
            payloads.push(payload);
        }

        encoded.push(records::assemble(
            schema,
            &shape,
            request.values(),
            payloads,
            end,
            None,
            &mut reader,
        )?);
    }

    check(reader.budget, cancelled)?;
    Ok(encoded)
}

pub(super) fn capacities(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Capacity>, Space), EditError> {
    let schema = source.table.schema();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, schema.layout.usage)?;
    let definition = definition::read(&mut reader, schema.definition_page())?;
    let reference = u32_at(&definition, TABLE_FREE_MAP_OFFSET)?;
    let hint = RepresentedMap::read(&mut reader, reference, MapRole::TableFreeSpace)?;
    let space = Space::capture_normal(image, schema, &owned, reader.budget, cancelled)?;
    reader
        .budget
        .charge_allocation(owned.len() as u64, size_of::<Capacity>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(owned.len())
        .map_err(|_| InspectError::Allocation)?;

    let mut previous = None;
    for number in owned {
        reader.tick()?;
        if previous.is_some_and(|previous| number <= previous) {
            return Err(EditError::Preservation);
        }
        previous = Some(number);
        space.require_page(image, number, false, reader.budget, cancelled)?;
        let eligible = match hint.cell(&mut reader, number) {
            Ok(cell) => cell.before & cell.mask != 0,
            Err(InspectError::Unsupported) => false,
            Err(error) => return Err(error.into()),
        };

        let page = reader.page(number)?;
        if page[..SIGNATURE.len()] != SIGNATURE
            || u32_at(&page, OWNER_AT)? != schema.definition_page()
        {
            return Err(EditError::Preservation);
        }
        let count = usize::from(u16_at(&page, COUNT_AT)?);
        if count > MAX_SLOTS {
            return Err(EditError::Preservation);
        }
        let directory_end = DIRECTORY_AT + count * DIRECTORY_BYTES;
        let mut frontier = PAGE_BYTES;
        for slot in 0..count {
            reader.tick()?;
            reader.work(3 * DIRECTORY_BYTES)?;
            let word = u16_at(&page, DIRECTORY_AT + slot * DIRECTORY_BYTES)?;
            let start = usize::from(word & OFFSET_MASK);
            if word & UNSUPPORTED_MASK != 0
                || start < directory_end
                || start > frontier
                || (start == frontier && word & DELETED_MASK == 0)
            {
                return Err(EditError::Preservation);
            }
            frontier = start;
        }
        let free = usize::from(u16_at(&page, FREE_AT)?);
        if frontier.checked_sub(directory_end) != Some(free) {
            return Err(EditError::Preservation);
        }

        result.push(Capacity {
            number,
            count,
            end: frontier,
            free,
            eligible,
        });
    }

    check(reader.budget, cancelled)?;
    Ok((result, space))
}
