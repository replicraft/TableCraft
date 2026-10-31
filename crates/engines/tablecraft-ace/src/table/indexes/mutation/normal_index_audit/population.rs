//! Raw original roots and complete freshly evaluated Int32 rows.
use super::vector;
use crate::{
    BeforeChangeFieldPreparation, EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, live_row_range, row_count},
        row::{COLUMN_COUNT_BYTES, MASK_BITS, OFFSET_MASK, reference},
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Row {
    pub reference: u32,
    pub before: Vec<i32>,
    pub after: Vec<i32>,
}

pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    prepared: &BeforeChangeFieldPreparation<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Row>, EditError> {
    let finals = prepared.final_rows().ok_or(EditError::Preservation)?;
    for pair in finals.windows(2) {
        check(budget, cancelled)?;
        if pair[0].ordinal() >= pair[1].ordinal() {
            return Err(EditError::Preservation);
        }
    }
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.native_type() != INT32
            || !column.fixed()
            || column.storage_length() != size_of::<i32>()
            || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || column.extended_flags() != 0
            || column.number() != position
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let end = column
            .field
            .offset
            .checked_add(size_of::<i32>())
            .ok_or(EditError::Preservation)?;
        for prior in &schema.columns()[..position] {
            check(budget, cancelled)?;
            if prior.field.offset < end
                && column.field.offset < prior.field.offset + size_of::<i32>()
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    if schema.columns().len() != schema.layout.max_columns {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let count = schema.declared_rows() as usize;
    if count > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut rows = vector(count, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let pages = allocation::pages(&mut reader, schema.layout.usage)?;
    let mut selected = 0;
    for number in pages {
        let raw = reader.page(number)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&raw, OWNER_OFFSET)? != schema.definition_page()
        {
            return Err(InspectError::Malformed.into());
        }
        for slot in 0..row_count(&raw)? {
            reader.tick()?;
            let Some((flags, start, end)) = live_row_range(&raw, slot)? else {
                continue;
            };
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            if flags & !OFFSET_MASK != 0 || rows.len() >= count {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let data = &raw[start..end];
            if usize::from(u16_at(data, 0)?) != schema.layout.max_columns {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let mask = data
                .len()
                .checked_sub(schema.layout.max_columns.div_ceil(MASK_BITS))
                .ok_or(InspectError::Malformed)?;
            let mut before = vector(schema.columns().len(), reader.budget, cancelled)?;
            let mut after = vector(schema.columns().len(), reader.budget, cancelled)?;
            let ordinal = rows.len();
            let final_row = finals.get(selected).filter(|row| row.ordinal() == ordinal);
            if final_row.is_some_and(|row| row.values().len() != schema.columns().len()) {
                return Err(EditError::Preservation);
            }
            for (position, column) in schema.columns().iter().enumerate() {
                reader.tick()?;
                let begin = COLUMN_COUNT_BYTES + column.field.offset;
                let finish = begin
                    .checked_add(size_of::<i32>())
                    .ok_or(EditError::Preservation)?;
                if finish > mask
                    || data
                        .get(mask + column.number() / MASK_BITS)
                        .is_none_or(|byte| byte & (1 << (column.number() % MASK_BITS)) == 0)
                {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                let value = i32::from_le_bytes(
                    data[begin..finish]
                        .try_into()
                        .map_err(|_| InspectError::Malformed)?,
                );
                before.push(value);
                let replacement = match final_row {
                    None => value,
                    Some(row) => match row.values().get(position) {
                        Some(Value::Int32(value)) => *value,
                        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
                    },
                };
                after.push(replacement);
            }
            if final_row.is_some() {
                selected += 1;
            }
            let slot = u8::try_from(slot).map_err(|_| InspectError::Malformed)?;
            rows.push(Row {
                reference: reference(number, slot),
                before,
                after,
            });
        }
    }
    if rows.len() != count || selected != finals.len() {
        return Err(EditError::Preservation);
    }
    check(reader.budget, cancelled)?;
    Ok(rows)
}
