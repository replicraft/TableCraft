//! Preserve complete native record framing while replacing one variable Name.
use super::{TableRenameError, source::Qualified};
use crate::{
    InspectError, NativeImage, NativeObjectOrigin, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        reader::row_range,
        row::{MASK_BITS, OFFSET_MASK, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES},
        text,
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const INLINE_TAG: u32 = 0x8000_0000;
const INLINE_HEADER_BYTES: usize = 3 * size_of::<u32>();

#[derive(Clone, Copy)]
pub(super) enum Payload<'a> {
    Absent,
    Inline(&'a [u8]),
    External(&'a super::creator_external::Binding),
}

pub(super) fn qualify_payloads(
    image: &NativeImage,
    schema: &TableSchema,
    origin: &NativeObjectOrigin,
    payload: Payload<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    check(budget, cancelled)?;
    let mut cursor = image.reader();
    let mut reader = crate::format::reader::Reader {
        source: &mut cursor,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(origin.page())?;
    let (raw, start, end) = row_range(&page, usize::from(origin.slot()))?;
    if raw & !OFFSET_MASK != 0 {
        return Err(TableRenameError::Source);
    }
    let row = &page[start..end];
    let framed = record_layout::parse(row, schema, reader.budget)?;
    if framed.count != schema.layout.max_columns
        || framed.offsets.len() != schema.layout.variable_slots + 1
    {
        return Err(TableRenameError::Source);
    }
    for column in schema.columns() {
        let field = &column.field;
        reader.tick()?;
        if field.fixed
            || row[framed.mask_start + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS))
                == 0
        {
            continue;
        }
        if field.name.eq_ignore_ascii_case("Name") {
            continue;
        }
        if !field.name.eq_ignore_ascii_case("LvProp") {
            return Err(TableRenameError::Dependency);
        }
        let start = framed.offsets[field.variable];
        let end = framed.offsets[field.variable + 1];
        let value = &row[start..end];
        match payload {
            Payload::Absent => return Err(TableRenameError::Dependency),
            Payload::External(binding) => binding.descriptor(value, reader.budget, cancelled)?,
            Payload::Inline(properties) => {
                let length =
                    u32::try_from(properties.len()).map_err(|_| InspectError::StructureLimit)?;
                if value.len() != INLINE_HEADER_BYTES + properties.len()
                    || u32_at(value, 0)? != INLINE_TAG | length
                    || value[size_of::<u32>()..INLINE_HEADER_BYTES]
                        .iter()
                        .any(|byte| *byte != 0)
                    || value[INLINE_HEADER_BYTES..] != *properties
                {
                    return Err(TableRenameError::Dependency);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn replace_name(
    image: &NativeImage,
    qualified: &Qualified,
    encoded: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, TableRenameError> {
    check(budget, cancelled)?;
    let page_start = qualified.identity.catalog_page() as usize * crate::format::page::PAGE_BYTES;
    let page = &image.as_bytes()[page_start..page_start + crate::format::page::PAGE_BYTES];
    let (_, start, end) = row_range(page, usize::from(qualified.identity.catalog_slot()))?;
    let original = &page[start..end];
    let framing = record_layout::parse(original, &qualified.catalog_schema, budget)?;
    let variable = qualified.name_column().field.variable;
    let name_start = framing.offsets[variable];
    let name_end = framing.offsets[variable + 1];
    let new_length = original
        .len()
        .checked_sub(name_end - name_start)
        .and_then(|length| length.checked_add(encoded.len()))
        .ok_or(TableRenameError::Capacity)?;
    if new_length > crate::format::page::PAGE_BYTES {
        return Err(TableRenameError::Capacity);
    }
    budget
        .charge_allocation(new_length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, new_length as u64)
        .map_err(InspectError::from)?;
    let mut row = Vec::new();
    row.try_reserve_exact(new_length)
        .map_err(|_| InspectError::Allocation)?;
    row.extend_from_slice(&original[..name_start]);
    row.extend_from_slice(encoded);
    row.extend_from_slice(&original[name_end..]);

    let count_position = row.len() - (original.len() - framing.mask_start) - VARIABLE_COUNT_BYTES;
    for (index, offset) in framing.offsets.iter().enumerate() {
        check(budget, cancelled)?;
        let shifted = if index <= variable {
            *offset
        } else {
            offset
                .checked_sub(name_end - name_start)
                .and_then(|offset| offset.checked_add(encoded.len()))
                .ok_or(TableRenameError::Capacity)?
        };
        let position = count_position - VARIABLE_OFFSET_BYTES * (index + 1);
        let stored = u16::try_from(shifted).map_err(|_| TableRenameError::Capacity)?;
        row[position..position + VARIABLE_OFFSET_BYTES].copy_from_slice(&stored.to_le_bytes());
    }
    verify(original, &row, qualified, budget, cancelled)?;
    Ok(row)
}

fn verify(
    before: &[u8],
    after: &[u8],
    qualified: &Qualified,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    let old = record_layout::parse(before, &qualified.catalog_schema, budget)?;
    let new = record_layout::parse(after, &qualified.catalog_schema, budget)?;
    let name = qualified.name_column().field.variable;
    if old.count != new.count
        || old.offsets.len() != new.offsets.len()
        || before[..old.offsets[0]] != after[..new.offsets[0]]
        || before[old.mask_start..] != after[new.mask_start..]
        || u16_at(before, old.mask_start - VARIABLE_COUNT_BYTES)?
            != u16_at(after, new.mask_start - VARIABLE_COUNT_BYTES)?
    {
        return Err(TableRenameError::Preservation);
    }
    for variable in 0..old.offsets.len() - 1 {
        check(budget, cancelled)?;
        if variable != name
            && before[old.offsets[variable]..old.offsets[variable + 1]]
                != after[new.offsets[variable]..new.offsets[variable + 1]]
        {
            return Err(TableRenameError::Preservation);
        }
    }
    if before[*old.offsets.last().ok_or(TableRenameError::Source)?..old.data_end]
        != after[*new.offsets.last().ok_or(TableRenameError::Source)?..new.data_end]
    {
        return Err(TableRenameError::Preservation);
    }
    // A separate decoder observes the actual proposed spelling; the encoder's
    // successful return alone does not establish a valid stored Name.
    let decoded = text::decode(
        &after[new.offsets[name]..new.offsets[name + 1]],
        true,
        budget,
    )?;
    if decoded != qualified.proposed {
        return Err(TableRenameError::Preservation);
    }
    Ok(())
}

/// Select an explicit planner from the real stored LvProp carrier only.
/// This does not qualify its body, properties, dependencies or mutation scope.
pub(super) fn database_properties_external(
    image: &NativeImage,
    schema: &TableSchema,
    origin: &NativeObjectOrigin,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, TableRenameError> {
    check(budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = crate::format::reader::Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(origin.page())?;
    let (raw, start, end) = row_range(&page, usize::from(origin.slot()))?;
    if raw & !OFFSET_MASK != 0 {
        return Err(TableRenameError::Source);
    }
    let row = &page[start..end];
    let framed = record_layout::parse(row, schema, reader.budget)?;
    let mut property = None;
    for column in schema.columns() {
        reader.work(column.name().len())?;
        if column.name().eq_ignore_ascii_case("LvProp") && property.replace(column).is_some() {
            return Err(TableRenameError::Source);
        }
    }
    let column = property.ok_or(TableRenameError::Source)?;
    let field = &column.field;
    if field.fixed
        || column.native_type() != crate::format::data_type::OLE
        || framed.count != schema.layout.max_columns
        || framed.offsets.len() != schema.layout.variable_slots + 1
        || field.number >= framed.count
        || field.variable >= schema.layout.variable_slots
    {
        return Err(TableRenameError::Source);
    }
    if row[framed.mask_start + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS)) == 0 {
        check(reader.budget, cancelled)?;
        return Ok(false);
    }
    let descriptor = row
        .get(framed.offsets[field.variable]..framed.offsets[field.variable + 1])
        .ok_or(TableRenameError::Source)?;
    reader.work(descriptor.len())?;
    let external = matches!(
        crate::table::long_value::trace::kind(descriptor)?,
        crate::table::long_value::trace::Kind::Single
    );
    check(reader.budget, cancelled)?;
    Ok(external)
}
