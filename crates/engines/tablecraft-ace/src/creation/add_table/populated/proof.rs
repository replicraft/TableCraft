//! Decode complete fresh roles and compare exact requested values independently.
use super::rows::Prepared;
use crate::{
    NativeImage, TableCheck,
    creation::add_table::AddTableError,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        layout,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count},
        row::{OFFSET_MASK, slot_offset},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const LAST_ISSUED_OFFSET: usize = 20;
const FREE_OFFSET: usize = 2;
const MAX_SLOTS: usize = u8::MAX as usize + 1;

pub(in crate::creation::add_table) fn verify(
    image: &NativeImage,
    checked: &TableCheck,
    expected: &Prepared<'_>,
    data_start: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    if !checked.indexes().is_empty() || checked.rows() != expected.rows.count() {
        return Err(AddTableError::Preservation);
    }
    let schema = checked.schema();
    {
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let definition = reader.page(schema.definition_page())?;
        if u32_at(&definition, LAST_ISSUED_OFFSET)? != 0 {
            return Err(AddTableError::Preservation);
        }
        let layout = layout::read(&mut reader, schema.definition_page())?;
        let owned = allocation::pages(&mut reader, layout.usage)?;
        let candidate = allocation::pages(
            &mut reader,
            u32_at(&definition, layout::TABLE_FREE_MAP_OFFSET)?,
        )?;
        let mut total = 0_usize;
        if owned.len() != expected.pages.len() {
            return Err(AddTableError::Preservation);
        }
        let mut candidates = candidate.iter().copied();
        for (offset, number) in owned.into_iter().enumerate() {
            reader.tick()?;
            if number as usize != data_start + offset {
                return Err(AddTableError::Preservation);
            }
            let page = reader.page(number)?;
            let count = row_count(&page)?;
            if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
                || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
                || count > MAX_SLOTS
            {
                return Err(AddTableError::Preservation);
            }
            let start = if count == 0 {
                PAGE_BYTES
            } else {
                usize::from(u16_at(&page, slot_offset(count - 1))? & OFFSET_MASK)
            };
            if start.checked_sub(slot_offset(count))
                != Some(usize::from(u16_at(&page, FREE_OFFSET)?))
            {
                return Err(AddTableError::Preservation);
            }
            let fits = count < MAX_SLOTS
                && usize::from(u16_at(&page, FREE_OFFSET)?)
                    >= expected.minimum_row + size_of::<u16>();
            if fits && candidates.next() != Some(number) {
                return Err(AddTableError::Preservation);
            }
            total = total
                .checked_add(count)
                .ok_or(AddTableError::Preservation)?;
            for slot in 0..count {
                reader.tick()?;
                if u16_at(&page, slot_offset(slot))? & !OFFSET_MASK != 0 {
                    return Err(AddTableError::Preservation);
                }
            }
        }
        if total != expected.rows.count() || candidates.next().is_some() {
            return Err(AddTableError::Preservation);
        }
    }
    let actual = crate::read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if actual.rows().len() != expected.rows.count() {
        return Err(AddTableError::Preservation);
    }
    for (index, row) in actual.rows().iter().enumerate() {
        check(budget, cancelled)?;
        if row.len() != expected.rows.width(index) {
            return Err(AddTableError::Preservation);
        }
        for (column, value) in row.iter().enumerate() {
            check(budget, cancelled)?;
            let crate::creation::input::Cell::Value(expected) = expected.rows.cell(index, column)
            else {
                return Err(AddTableError::Preservation);
            };
            let work = match (value, expected) {
                (Value::Text(a), Value::Text(b)) => a.len().max(b.len()),
                (Value::Binary(a), Value::Binary(b)) => a.len().max(b.len()),
                _ => 1,
            };
            budget
                .charge(Resource::Steps, work as u64)
                .map_err(crate::InspectError::from)?;
            if value != expected {
                return Err(AddTableError::Preservation);
            }
        }
    }
    Ok(())
}
