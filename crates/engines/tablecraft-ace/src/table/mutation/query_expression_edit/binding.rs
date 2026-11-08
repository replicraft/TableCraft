//! Resolve inert expression keys to native logical ROWs, without write authority.
//!
//! This leaf does not qualify catalog properties, indexes, page roles or writable
//! storage. Its caller must retain those independent source proofs, then pass the
//! resolved ordinal assignments through `record_roots::capture` before placement.
//! Neither a decoded fragment position nor a physical hidden slot is a selector.

use crate::{
    EditError, InspectError, NativeImage, QueryAttribute, TableSchema,
    format::{
        allocation,
        bytes::u32_at,
        data_type,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, live_row_range, row_count},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, reference},
    },
    header::check,
    limits::MAX_LIVE_ROWS,
    table::{mutation::rows::RowAssignment, record_layout},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[cfg(test)]
#[path = "binding_tests.rs"]
mod tests;

const FIELD_COUNT: usize = 8;
const VARIABLE_COUNT: usize = 4;
const ORDER_BYTES: usize = size_of::<u32>();
const INLINE_HEADER_BYTES: usize = 3 * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;

/// Original native identity and text, not an evaluated value or raw ROW address.
pub(super) struct Expected<'a> {
    pub native_id: i32,
    pub attribute: QueryAttribute,
    pub order: [u8; ORDER_BYTES],
    pub expression: &'a str,
}

/// A source-derived logical assignment. Placement must recheck the same root.
pub(super) struct Resolved {
    pub request: usize,
    pub reference: u32,
    ordinal: usize,
}

impl RowAssignment for Resolved {
    fn row(&self) -> usize {
        self.ordinal
    }
}

struct Columns {
    object: usize,
    attribute: usize,
    order: usize,
    expression: usize,
}

/// Resolve all requests exactly once over complete live logical membership.
///
/// Forwarding is decoded by the existing bounded native reader. Hidden retired
/// targets are not live roots, and cannot independently match an expression key.
/// This is selection data only; `record_roots::capture` owns exclusive final-row
/// ownership and retained placement callers own complete physical role exclusion.
pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    requests: &[Expected<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Resolved>, EditError> {
    check(budget, cancelled)?;
    let columns = columns(schema, budget, cancelled)?;
    let declared = usize::try_from(schema.declared_rows())
        .ok()
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;
    if requests.len() > declared {
        return Err(EditError::Conflict);
    }
    for (position, request) in requests.iter().enumerate() {
        check(budget, cancelled)?;
        if !matches!(
            request.attribute,
            QueryAttribute::Column | QueryAttribute::Where
        ) {
            return Err(InspectError::Unsupported.into());
        }
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if requests[..position].iter().any(|previous| {
            previous.native_id == request.native_id
                && previous.attribute == request.attribute
                && previous.order == request.order
        }) {
            return Err(EditError::Conflict);
        }
    }
    budget
        .charge_allocation(requests.len() as u64, size_of::<Resolved>() as u64)
        .map_err(InspectError::from)?;
    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let pages = allocation::pages(&mut reader, schema.layout.usage)?;
    let mut ordinal = 0;
    for number in pages {
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
        {
            return Err(InspectError::Malformed.into());
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            if live_row_range(&page, slot)?.is_none() {
                continue;
            }
            if ordinal >= declared {
                return Err(InspectError::Malformed.into());
            }
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let slot = u8::try_from(slot).map_err(|_| InspectError::Malformed)?;
            let physical = reader.row_location(number, slot, schema.definition_page(), &page)?;
            let row = physical.data.as_slice();
            let layout = record_layout::parse(row, schema, reader.budget)?;
            if layout.count != FIELD_COUNT || layout.offsets.len() != VARIABLE_COUNT + 1 {
                return Err(InspectError::Unsupported.into());
            }
            require_present(row, &layout, schema, columns.object)?;
            require_present(row, &layout, schema, columns.attribute)?;
            let native_id = u32_at(
                row,
                COLUMN_COUNT_BYTES + schema.columns()[columns.object].field.offset,
            )? as i32;
            let attribute = *row
                .get(COLUMN_COUNT_BYTES + schema.columns()[columns.attribute].field.offset)
                .ok_or(InspectError::Malformed)?;
            reader.work(requests.len())?;
            for (request, expected) in requests.iter().enumerate() {
                if native_id != expected.native_id || attribute != expected.attribute.code() {
                    continue;
                }
                require_present(row, &layout, schema, columns.order)?;
                let order = payload(row, &layout, schema, columns.order)?;
                if order != expected.order {
                    continue;
                }
                reader.work(resolved.len())?;
                if resolved
                    .iter()
                    .any(|binding: &Resolved| binding.request == request)
                {
                    return Err(EditError::Conflict);
                }
                require_present(row, &layout, schema, columns.expression)?;
                require_inline(
                    payload(row, &layout, schema, columns.expression)?,
                    expected.expression,
                    reader.budget,
                    cancelled,
                )?;
                resolved.push(Resolved {
                    request,
                    reference: reference(number, slot),
                    ordinal,
                });
            }
            ordinal += 1;
        }
    }
    if ordinal != declared || resolved.len() != requests.len() {
        return Err(EditError::Conflict);
    }
    check(reader.budget, cancelled)?;
    Ok(resolved)
}

fn columns(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    if schema.columns().len() != FIELD_COUNT
        || schema.layout.max_columns != FIELD_COUNT
        || schema.layout.variable_slots != VARIABLE_COUNT
    {
        return Err(InspectError::Unsupported);
    }
    let mut positions = [0; FIELD_COUNT];
    for (ordinal, (name, kind, fixed)) in [
        ("Attribute", data_type::BYTE, true),
        ("Expression", data_type::LONG_TEXT, false),
        ("Flag", data_type::INT16, true),
        ("LvExtra", data_type::INT32, true),
        ("Name1", data_type::SHORT_TEXT, false),
        ("Name2", data_type::SHORT_TEXT, false),
        ("ObjectId", data_type::INT32, true),
        ("Order", data_type::SHORT_BINARY, false),
    ]
    .into_iter()
    .enumerate()
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, FIELD_COUNT as u64)?;
        let (position, field) = schema
            .columns()
            .iter()
            .enumerate()
            .find(|(_, field)| field.name() == name)
            .ok_or(InspectError::Unsupported)?;
        if field.native_type() != kind || field.fixed() != fixed || field.extended_flags() != 0 {
            return Err(InspectError::Unsupported);
        }
        positions[ordinal] = position;
    }
    Ok(Columns {
        object: positions[6],
        attribute: positions[0],
        order: positions[7],
        expression: positions[1],
    })
}

fn require_present(
    row: &[u8],
    layout: &record_layout::RecordLayout,
    schema: &TableSchema,
    column: usize,
) -> Result<(), InspectError> {
    let number = schema.columns()[column].number();
    let byte = row
        .get(layout.mask_start + number / MASK_BITS)
        .ok_or(InspectError::Malformed)?;
    if byte & (1 << (number % MASK_BITS)) == 0 {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}

fn payload<'a>(
    row: &'a [u8],
    layout: &record_layout::RecordLayout,
    schema: &TableSchema,
    column: usize,
) -> Result<&'a [u8], InspectError> {
    let variable = schema.columns()[column].field.variable;
    let start = *layout
        .offsets
        .get(variable)
        .ok_or(InspectError::Malformed)?;
    let end = *layout
        .offsets
        .get(variable + 1)
        .ok_or(InspectError::Malformed)?;
    row.get(start..end).ok_or(InspectError::Malformed)
}

fn require_inline(
    stored: &[u8],
    expected: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut units = 0_usize;
    for _ in expected.encode_utf16() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        units = units.checked_add(1).ok_or(InspectError::StructureLimit)?;
    }
    let length = units
        .checked_mul(size_of::<u16>())
        .ok_or(InspectError::StructureLimit)?;
    let length_word = u32::try_from(length).map_err(|_| InspectError::StructureLimit)?;
    if stored.len() != INLINE_HEADER_BYTES + length
        || u32_at(stored, 0)? != INLINE_TAG | length_word
        || stored[size_of::<u32>()..INLINE_HEADER_BYTES]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(EditError::Conflict);
    }
    for (bytes, unit) in stored[INLINE_HEADER_BYTES..]
        .as_chunks::<{ size_of::<u16>() }>()
        .0
        .iter()
        .zip(expected.encode_utf16())
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        if *bytes != unit.to_le_bytes() {
            return Err(EditError::Conflict);
        }
    }
    Ok(())
}
