//! Rebuild one selected variable field while retaining physical capacity on fit.
use super::super::{EditError, WriteBlocker, slot::storage};
use crate::{
    InspectError, NativeColumn, TableSchema,
    format::{
        data_type::SHORT_TEXT,
        layout::UNICODE_COMPRESSION_FLAG,
        row::{MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
        text,
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const UTF16_BYTES: usize = size_of::<u16>();
const COMPRESSION_MARKER_BYTES: usize = 2;

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let frame = Frame::read(schema, column, before, budget)?;
    let payload = preferred(&schema.columns[column], value, budget, cancelled)?;
    replace(
        frame,
        &payload,
        !matches!(value, Value::Null),
        budget,
        cancelled,
    )
}

/// Replace an already qualified variable-field payload without changing its
/// type-specific serialization policy. Both working callers retain their own
/// field/value admission and independent final semantic checks.
pub(in crate::table::mutation) fn replace_payload(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    payload: &[u8],
    present: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let frame = Frame::read(schema, column, before, budget)?;
    replace(frame, payload, present, budget, cancelled)
}

struct Frame<'a> {
    schema: &'a TableSchema,
    column: usize,
    before: &'a [u8],
    layout: record_layout::RecordLayout,
    start: usize,
    end: usize,
    payload_end: usize,
}

impl<'a> Frame<'a> {
    fn read(
        schema: &'a TableSchema,
        column: usize,
        before: &'a [u8],
        budget: &mut ResourceBudget,
    ) -> Result<Self, EditError> {
        let field = schema.columns.get(column).ok_or(InspectError::Malformed)?;
        let layout = record_layout::parse(before, schema, budget)?;
        if layout.count != schema.layout.max_columns {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let variable = field.field.variable;
        let start = *layout
            .offsets
            .get(variable)
            .ok_or(InspectError::Malformed)?;
        let end = *layout
            .offsets
            .get(variable + 1)
            .ok_or(InspectError::Malformed)?;
        let payload_end = *layout.offsets.last().ok_or(InspectError::Malformed)?;

        Ok(Self {
            schema,
            column,
            before,
            layout,
            start,
            end,
            payload_end,
        })
    }
}

fn replace(
    frame: Frame<'_>,
    payload: &[u8],
    present: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let Frame {
        schema,
        column,
        before,
        layout,
        start,
        end,
        payload_end,
    } = frame;
    let field = &schema.columns[column];
    let variable = field.field.variable;
    let following = payload_end
        .checked_sub(end)
        .ok_or(InspectError::Malformed)?;
    let logical_end = start
        .checked_add(payload.len())
        .and_then(|end| end.checked_add(following))
        .ok_or(InspectError::StructureLimit)?;
    let footer = before
        .len()
        .checked_sub(layout.data_end)
        .ok_or(InspectError::Malformed)?;
    let length = before.len().max(
        logical_end
            .checked_add(footer)
            .ok_or(InspectError::StructureLimit)?,
    );
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    after.extend_from_slice(before);
    after.resize(length, 0);

    // Immutable source bytes make overlapping growth/shrink safe. Other field
    // payloads are copied exactly; unconsumed original padding stays intact.
    after[start..start + payload.len()].copy_from_slice(payload);
    after[start + payload.len()..logical_end].copy_from_slice(&before[end..payload_end]);
    let mask_bytes = layout.count.div_ceil(MASK_BITS);
    let mask = length
        .checked_sub(mask_bytes)
        .ok_or(InspectError::Malformed)?;
    after[mask..].copy_from_slice(&before[layout.mask_start..]);
    let bit = 1_u8 << (field.number() % MASK_BITS);
    let mask_position = mask + field.number() / MASK_BITS;
    after[mask_position] = (after[mask_position] & !bit) | if present { bit } else { 0 };
    let count_position = mask
        .checked_sub(VARIABLE_COUNT_BYTES)
        .ok_or(InspectError::Malformed)?;
    let variables = layout
        .offsets
        .len()
        .checked_sub(1)
        .ok_or(InspectError::Malformed)?;
    let count = u16::try_from(variables).map_err(|_| InspectError::StructureLimit)?;
    after[count_position..count_position + VARIABLE_COUNT_BYTES]
        .copy_from_slice(&count.to_le_bytes());
    for (position, original) in layout.offsets.iter().copied().enumerate() {
        check(budget, cancelled)?;
        let offset = if position <= variable {
            original
        } else {
            original
                .checked_sub(end - start)
                .and_then(|offset| offset.checked_add(payload.len()))
                .ok_or(InspectError::StructureLimit)?
        };
        let encoded = u16::try_from(offset).map_err(|_| InspectError::StructureLimit)?;
        let at = count_position
            .checked_sub(VARIABLE_OFFSET_BYTES * (position + 1))
            .ok_or(InspectError::Malformed)?;
        after[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&encoded.to_le_bytes());
    }
    record_layout::parse(&after, schema, budget)?;
    Ok(after)
}

fn preferred(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    storage::validate(field, value)?;
    check(budget, cancelled)?;
    let length = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Binary(bytes) => bytes.len(),
        Value::Text(value) => {
            let count = value.encode_utf16().count();
            let compact = field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
                && count > COMPRESSION_MARKER_BYTES
                && value
                    .encode_utf16()
                    .all(|unit| unit > 0 && unit <= u16::from(u8::MAX));
            if compact {
                COMPRESSION_MARKER_BYTES + count
            } else {
                UTF16_BYTES * count
            }
        }
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    // The exact-span chooser permits either valid form on equal size. Preferred
    // row serialization uses raw UTF16 for the two-unit compression tie, as the
    // independent public update observations establish. Never mask metadata to
    // borrow a different encoder profile.
    if field.native_type() == SHORT_TEXT
        && field.extended_flags() & UNICODE_COMPRESSION_FLAG != 0
        && let Value::Text(value) = value
        && value.encode_utf16().count() == COMPRESSION_MARKER_BYTES
    {
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        for unit in value.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        if text::decode(&bytes, true, budget)? != *value {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        return Ok(bytes);
    }
    storage::encode_for_span(field, value, length, budget, cancelled)
}
