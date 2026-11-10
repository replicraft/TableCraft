//! Independent append reconstruction keeps retired bytes uninterpreted.
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset,
        },
    },
    header::check,
    table::{record_layout, row_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let original = record_layout::parse_values(before, schema, budget)?;
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    if !row_layout::geometry(schema, original.count)?.ambiguous_tail() {
        return Err(EditError::Preservation);
    }
    let field = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    if field.native_type() != INT32
        || !field.fixed()
        || field.storage_length() != size_of::<i32>()
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || field.extended_flags() != 0
        || !matches!(value, Value::Int32(_) | Value::Null)
    {
        return Err(EditError::Preservation);
    }
    let selected = COLUMN_COUNT_BYTES
        .checked_add(field.physical_field().offset)
        .ok_or(InspectError::StructureLimit)?;
    let stop = selected
        .checked_add(size_of::<i32>())
        .ok_or(InspectError::StructureLimit)?;
    if stop > schema.physical_layout().fixed_end {
        return Err(EditError::Preservation);
    }
    for (position, live) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if !live.fixed() && live.number() < original.count {
            return Err(EditError::Preservation);
        }
        let left = COLUMN_COUNT_BYTES
            .checked_add(live.physical_field().offset)
            .ok_or(InspectError::StructureLimit)?;
        let right = left
            .checked_add(live.storage_length())
            .ok_or(InspectError::StructureLimit)?;
        if position != column && live.fixed() && selected < right && left < stop {
            return Err(EditError::Preservation);
        }
    }
    let fields = schema.physical_layout().max_columns;
    let variables = schema.physical_layout().variable_slots;
    let prefix = original.mask_start.max(schema.physical_layout().fixed_end);
    let footer = if variables == 0 {
        0
    } else {
        variables
            .checked_add(1)
            .and_then(|words| words.checked_mul(VARIABLE_OFFSET_BYTES))
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let length = prefix
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(fields.div_ceil(MASK_BITS)))
        .ok_or(InspectError::StructureLimit)?;
    if length > PAGE_BYTES - slot_offset(1) {
        return Err(EditError::Preservation);
    }
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, (length + original.mask_start) as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend_from_slice(&before[..original.mask_start]);
    expected.resize(prefix, 0);
    expected[..COLUMN_COUNT_BYTES].copy_from_slice(
        &u16::try_from(fields)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    if let Value::Int32(value) = value {
        expected[selected..stop].copy_from_slice(&value.to_le_bytes());
    }
    if variables != 0 {
        let offset = u16::try_from(prefix)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes();
        for _ in 0..=variables {
            check(budget, cancelled)?;
            expected.extend_from_slice(&offset);
        }
        expected.extend_from_slice(
            &u16::try_from(variables)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    let mask = expected.len();
    expected.extend_from_slice(&before[original.mask_start..]);
    expected.resize(length, 0);
    for logical in original.count..fields {
        check(budget, cancelled)?;
        expected[mask + logical / MASK_BITS] &= !(1 << (logical % MASK_BITS));
    }
    let selected_mask = mask + field.number() / MASK_BITS;
    let bit = 1 << (field.number() % MASK_BITS);
    expected[selected_mask] =
        (expected[selected_mask] & !bit) | if matches!(value, Value::Null) { 0 } else { bit };
    record_layout::parse(&expected, schema, budget)?;
    Ok(expected)
}
