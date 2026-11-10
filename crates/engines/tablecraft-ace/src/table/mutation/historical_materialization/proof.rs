//! Original bytes independently reconstruct the row and physical placement.
use super::{Selected, bind};
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        bytes::u16_at,
        row::{COLUMN_COUNT_BYTES, MASK_BITS, OFFSET_MASK},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::{
        mutation::{
            historical_fixed::source::Source,
            preserved_source::validation,
            record_placement::{proof as placement, space::Space},
            record_roots, record_serialization,
        },
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LONG_BYTES: usize = size_of::<i32>();

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = Source::capture(image, table, budget, cancelled)?;
    source
        .facts()
        .require_preserved_definition_tail(image, after, budget, cancelled)?;
    let requests = [bind(&source, groups, budget, cancelled)?];
    let schema = source.facts().table.schema();
    let captured = record_roots::capture(image, schema, &requests, budget, cancelled)?;
    let root = captured.selected.first().ok_or(EditError::Conflict)?;
    if captured.selected.len() != 1 || root.word & !OFFSET_MASK != 0 {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let record = reconstruct(
        schema,
        root.update,
        &root.final_record.data,
        groups[0].updates()[0].value(),
        budget,
        cancelled,
    )?;
    let space = Space::capture_normal(image, schema, &captured.owned, budget, cancelled)?;
    let mut physical =
        placement::Placement::capture(image, &space, &captured.owned, budget, cancelled)?;
    let choice = physical.reserve(root, &record, budget, cancelled)?;
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    placement::apply(
        image,
        &space,
        root,
        &record,
        &choice,
        &mut expected,
        budget,
        cancelled,
    )?;
    space.audit(image.as_bytes(), &expected, budget, cancelled)?;
    if expected.len() != after.len() {
        return Err(EditError::Preservation);
    }
    for (original, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, original.len() as u64)
            .map_err(InspectError::from)?;
        if original != actual {
            return Err(EditError::Preservation);
        }
    }
    validation::expected(
        image,
        table,
        &source.facts().table,
        &expected,
        budget,
        cancelled,
    )?;
    Ok(())
}

fn reconstruct(
    schema: &TableSchema,
    selected: &Selected,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let original = record_layout::parse(before, schema, budget)?;
    let count = usize::from(u16_at(before, 0)?);
    let fields = schema.columns().len();
    let Value::Int32(value) = value else {
        return Err(EditError::Preservation);
    };
    let capacity = COLUMN_COUNT_BYTES
        .checked_add(
            fields
                .checked_mul(LONG_BYTES)
                .ok_or(InspectError::StructureLimit)?,
        )
        .and_then(|bytes| bytes.checked_add(fields.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend(
        u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    for (number, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if number < count {
            let at = COLUMN_COUNT_BYTES + field.field.offset;
            let payload = before
                .get(at..at + LONG_BYTES)
                .ok_or(EditError::Preservation)?;
            expected.extend_from_slice(payload);
        } else if number == selected.column {
            expected.extend(value.to_le_bytes());
        } else {
            expected.extend([0; LONG_BYTES]);
        }
    }
    let old_mask = &before[original.mask_start..];
    for byte in 0..fields.div_ceil(MASK_BITS) {
        check(budget, cancelled)?;
        let mut bits = if byte < old_mask.len() {
            old_mask[byte]
        } else {
            0
        };
        for bit in 0..MASK_BITS {
            let number = byte * MASK_BITS + bit;
            if number >= count && number < fields {
                bits &= !(1 << bit);
                if number == selected.column {
                    bits |= 1 << bit;
                }
            }
        }
        expected.push(bits);
    }
    if expected.len() != capacity {
        return Err(EditError::Preservation);
    }
    Ok(expected)
}
