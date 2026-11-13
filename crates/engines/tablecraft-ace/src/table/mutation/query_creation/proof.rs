//! Fresh storage, independent requested-row reconstruction and literal complement.
use super::source::{self, Source};
use crate::{
    InspectError, NativeImage, QueryAttribute, QueryCreationDefinition, QueryCreationError,
    TableSchema,
    format::{
        bytes::u32_at,
        data_append::Page,
        data_type::{LONG_TEXT, SHORT_TEXT},
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
        row::{MASK_BITS, slot_offset},
    },
    header::check,
    read_saved_query_definition,
    table::{record as native_record, record_layout},
};
use arrayvec::ArrayString;
use std::{fmt::Write, io::Cursor, ops::Range};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const INTEGER_TEXT_BYTES: usize = i32::MIN.unsigned_abs().ilog10() as usize + 2;
const ATTRIBUTES: [QueryAttribute; 5] = [
    QueryAttribute::Start,
    QueryAttribute::Type,
    QueryAttribute::Flags,
    QueryAttribute::Column,
    QueryAttribute::End,
];
const INLINE_TAG: u32 = 0x8000_0000;
const MEMO_DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const ORDER: [u8; size_of::<u32>()] = [0, 0, 0, 1];
const SHORT_TEXT_ESCAPE: [u8; 3] = [0xff, 0xfe, 0];

pub(super) fn verify(
    before: &NativeImage,
    after: &NativeImage,
    original: &Source,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    check(budget, cancelled)?;
    let fresh = source::capture_after(after, budget, cancelled)?;
    let old = &original.storage;
    if before.as_bytes().len() != after.as_bytes().len()
        || fresh.parent != old.parent
        || fresh.catalog_page != old.catalog_page
        || fresh.query_page != old.query_page
        || fresh.catalog.definition_page() != old.catalog.definition_page()
        || fresh.queries.definition_page() != old.queries.definition_page()
        || old.catalog.declared_rows().checked_add(1) != Some(fresh.catalog.declared_rows())
        || old.queries.declared_rows().checked_add(5) != Some(fresh.queries.declared_rows())
    {
        return Err(QueryCreationError::Preservation);
    }

    let old_catalog = capture(
        before,
        old.catalog_page,
        old.catalog.definition_page(),
        budget,
        cancelled,
    )?;
    let new_catalog = capture(
        after,
        fresh.catalog_page,
        fresh.catalog.definition_page(),
        budget,
        cancelled,
    )?;
    let old_queries = capture(
        before,
        old.query_page,
        old.queries.definition_page(),
        budget,
        cancelled,
    )?;
    let new_queries = capture(
        after,
        fresh.query_page,
        fresh.queries.definition_page(),
        budget,
        cancelled,
    )?;
    budget.charge(
        Resource::Steps,
        (old_catalog.count + old_queries.count) as u64 * 2,
    )?;
    if new_catalog.count != old_catalog.count + 1
        || new_queries.count != old_queries.count + ATTRIBUTES.len()
        || new_catalog.records[..old_catalog.count] != old_catalog.records
        || new_queries.records[..old_queries.count] != old_queries.records
    {
        return Err(QueryCreationError::Preservation);
    }

    verify_record(
        after,
        &fresh.catalog,
        record(
            after,
            fresh.catalog_page,
            &new_catalog.records[old_catalog.count],
        )?,
        Expected {
            source: original,
            definition,
            attribute: None,
        },
        budget,
        cancelled,
    )?;
    for (ordinal, attribute) in ATTRIBUTES.into_iter().enumerate() {
        verify_record(
            after,
            &fresh.queries,
            record(
                after,
                fresh.query_page,
                &new_queries.records[old_queries.count + ordinal],
            )?,
            Expected {
                source: original,
                definition,
                attribute: Some(attribute),
            },
            budget,
            cancelled,
        )?;
    }

    let scope = after.catalog().catalog().scope();
    let saved = read_saved_query_definition(
        &mut after.reader(),
        scope,
        definition.name(),
        budget,
        cancelled,
    )?;
    let origin = saved.origin();
    budget.charge(
        Resource::Steps,
        (saved.object().name().as_str().len() + definition.name().len()) as u64,
    )?;
    if saved.object().name().as_str() != definition.name()
        || origin.native_id() != original.native_id
        || origin.parent_id() != old.parent
        || origin.type_code() != 5
        || origin.flags() != 0
        || origin.page() != fresh.catalog_page
        || usize::from(origin.slot()) != old_catalog.count
        || saved.rows().len() != ATTRIBUTES.len()
    {
        return Err(QueryCreationError::Preservation);
    }
    let mut literal = ArrayString::<INTEGER_TEXT_BYTES>::new();
    write!(literal, "{}", definition.value()).map_err(|_| QueryCreationError::Preservation)?;
    for (row, code) in saved.rows().iter().zip(ATTRIBUTES) {
        check(budget, cancelled)?;
        let compared = row.name1().map_or(0, str::len)
            + row.expression().map_or(0, str::len)
            + definition.alias().len()
            + literal.len()
            + ORDER.len();
        budget.charge(Resource::Steps, compared as u64 + 1)?;
        let column = code == QueryAttribute::Column;
        let flag = match code {
            QueryAttribute::Type => Some(1),
            QueryAttribute::Flags => Some(0),
            _ => None,
        };
        if row.native_object_id() != original.native_id
            || row.attribute() != Some(code)
            || row.flag() != flag
            || row.extra().is_some()
            || row.order() != Some(ORDER.as_slice())
            || row.name1() != column.then_some(definition.alias())
            || row.name2().is_some()
            || row.expression() != column.then_some(literal.as_str())
        {
            return Err(QueryCreationError::Preservation);
        }
    }

    let catalog_count = old.catalog.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let query_count = old.queries.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    for (at, (&left, &right)) in before.as_bytes().iter().zip(after.as_bytes()).enumerate() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        if left == right {
            continue;
        }
        let count = (catalog_count..catalog_count + size_of::<u32>()).contains(&at)
            || (query_count..query_count + size_of::<u32>()).contains(&at);
        let page_number = at / PAGE_BYTES;
        let offset = at % PAGE_BYTES;
        let append = page_number == old.catalog_page as usize
            && append_scope(offset, &old_catalog, &new_catalog)
            || page_number == old.query_page as usize
                && append_scope(offset, &old_queries, &new_queries);
        if !count && !append {
            return Err(QueryCreationError::Preservation);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Expected<'a> {
    source: &'a Source,
    definition: &'a QueryCreationDefinition,
    attribute: Option<QueryAttribute>,
}

impl Expected<'_> {
    fn matches(self, name: &str, actual: &Value, literal: &str) -> bool {
        match (self.attribute, name, actual) {
            (None, "Id", Value::Int32(value)) | (Some(_), "ObjectId", Value::Int32(value)) => {
                *value == self.source.native_id
            }
            (None, "ParentId", Value::Int32(value)) => *value == self.source.storage.parent,
            (None, "Type", Value::Int16(value)) => *value == 5,
            (None, "Flags", Value::Int32(value)) => *value == 0,
            (None, "DateCreate" | "DateUpdate", Value::DateTime(value)) => {
                *value == self.definition.captured_at()
            }
            (None, "Name", Value::Text(value)) => value == self.definition.name(),
            (Some(attribute), "Attribute", Value::Byte(value)) => *value == attribute.code(),
            (Some(QueryAttribute::Type), "Flag", Value::Int16(value)) => *value == 1,
            (Some(QueryAttribute::Flags), "Flag", Value::Int16(value)) => *value == 0,
            (Some(_), "Order", Value::Binary(value)) => value.as_slice() == ORDER,
            (Some(QueryAttribute::Column), "Name1", Value::Text(value)) => {
                value == self.definition.alias()
            }
            (Some(QueryAttribute::Column), "Expression", Value::Text(value)) => value == literal,
            (_, _, Value::Null) => !matches!(
                (self.attribute, name),
                (
                    None,
                    "Id" | "ParentId" | "Type" | "Flags" | "DateCreate" | "DateUpdate" | "Name"
                ) | (Some(_), "ObjectId" | "Attribute" | "Order")
                    | (Some(QueryAttribute::Type | QueryAttribute::Flags), "Flag")
                    | (Some(QueryAttribute::Column), "Name1" | "Expression")
            ),
            _ => false,
        }
    }
}

fn verify_record(
    image: &NativeImage,
    schema: &TableSchema,
    actual: &[u8],
    expected: Expected<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    let frame = record_layout::parse(actual, schema, budget)?;
    if frame.count != schema.layout.max_columns
        || frame.offsets.len() != schema.layout.variable_slots + 1
        || frame.offsets.first() != Some(&schema.layout.fixed_end)
        || frame.offsets.last() != Some(&frame.data_end)
    {
        return Err(QueryCreationError::Preservation);
    }
    let mut input = Cursor::new(image.as_bytes());
    let values = {
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        native_record::decode(actual, schema, &mut reader)?
    };
    if values.len() != schema.columns().len() {
        return Err(QueryCreationError::Preservation);
    }
    let mut literal = ArrayString::<INTEGER_TEXT_BYTES>::new();
    write!(literal, "{}", expected.definition.value())
        .map_err(|_| QueryCreationError::Preservation)?;
    for (field, value) in schema.columns().iter().zip(&values) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, field.name().len() as u64 + 1)?;
        let compared = match value {
            Value::Text(text) => {
                text.len()
                    + match field.name() {
                        "Name" => expected.definition.name().len(),
                        "Name1" => expected.definition.alias().len(),
                        "Expression" => literal.len(),
                        _ => 0,
                    }
            }
            Value::Binary(bytes) => bytes.len() + ORDER.len(),
            _ => 0,
        };
        budget.charge(Resource::Steps, compared as u64 + 1)?;
        if !expected.matches(field.name(), value, &literal) {
            return Err(QueryCreationError::Preservation);
        }
        if field.fixed() && matches!(value, Value::Null) {
            let start = size_of::<u16>() + field.field.offset;
            let end = start
                .checked_add(field.storage_length())
                .ok_or(InspectError::StructureLimit)?;
            budget.charge(Resource::Steps, field.storage_length() as u64)?;
            if actual
                .get(start..end)
                .ok_or(QueryCreationError::Preservation)?
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(QueryCreationError::Preservation);
            }
        } else if !field.fixed() {
            let start = *frame
                .offsets
                .get(field.field.variable)
                .ok_or(QueryCreationError::Preservation)?;
            let end = *frame
                .offsets
                .get(field.field.variable + 1)
                .ok_or(QueryCreationError::Preservation)?;
            let payload = actual
                .get(start..end)
                .ok_or(QueryCreationError::Preservation)?;
            if matches!(value, Value::Null) && !payload.is_empty() {
                return Err(QueryCreationError::Preservation);
            }
            if field.native_type() == SHORT_TEXT
                && let Value::Text(text) = value
            {
                verify_short_text(payload, text, budget, cancelled)?;
            }
            if field.native_type() == LONG_TEXT && !matches!(value, Value::Null) {
                let Value::Text(text) = value else {
                    return Err(QueryCreationError::Preservation);
                };
                budget.charge(Resource::Steps, (text.len() + MEMO_DESCRIPTOR_BYTES) as u64)?;
                let length = text
                    .encode_utf16()
                    .count()
                    .checked_mul(size_of::<u16>())
                    .ok_or(InspectError::StructureLimit)?;
                let native_length =
                    u32::try_from(length).map_err(|_| InspectError::StructureLimit)?;
                if payload.len() != MEMO_DESCRIPTOR_BYTES + length
                    || u32_at(payload, 0)? != (INLINE_TAG | native_length)
                    || payload[size_of::<u32>()..MEMO_DESCRIPTOR_BYTES]
                        .iter()
                        .any(|byte| *byte != 0)
                {
                    return Err(QueryCreationError::Preservation);
                }
                for (bytes, unit) in payload[MEMO_DESCRIPTOR_BYTES..]
                    .chunks_exact(size_of::<u16>())
                    .zip(text.encode_utf16())
                {
                    check(budget, cancelled)?;
                    budget.charge(Resource::Steps, size_of::<u16>() as u64)?;
                    if bytes != unit.to_le_bytes() {
                        return Err(QueryCreationError::Preservation);
                    }
                }
            }
        }
    }
    // Complete declaration spans own every fixed byte; unassigned padding and
    // unused null-mask bits may not hide an extra caller-supplied payload.
    for (offset, byte) in actual
        .iter()
        .enumerate()
        .take(schema.layout.fixed_end)
        .skip(size_of::<u16>())
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, schema.columns().len() as u64)?;
        let assigned = schema.columns().iter().any(|field| {
            let start = size_of::<u16>() + field.field.offset;
            field.fixed() && (start..start + field.storage_length()).contains(&offset)
        });
        if !assigned && *byte != 0 {
            return Err(QueryCreationError::Preservation);
        }
    }
    for number in schema.layout.max_columns..(actual.len() - frame.mask_start) * MASK_BITS {
        if actual[frame.mask_start + number / MASK_BITS] & (1 << (number % MASK_BITS)) != 0 {
            return Err(QueryCreationError::Preservation);
        }
    }
    Ok(())
}

/// Source qualification requires uncompressed Short Text declarations. Check
/// exact preferred bytes independently, including the leading-marker escape.
fn verify_short_text(
    payload: &[u8],
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, text.len() as u64)?;
    let escaped = text.starts_with('\u{feff}');
    let prefix = if escaped { SHORT_TEXT_ESCAPE.len() } else { 0 };
    let length = text
        .encode_utf16()
        .count()
        .checked_mul(size_of::<u16>())
        .and_then(|bytes| bytes.checked_add(prefix))
        .ok_or(InspectError::StructureLimit)?;
    if payload.len() != length || escaped && !payload.starts_with(&SHORT_TEXT_ESCAPE) {
        return Err(QueryCreationError::Preservation);
    }
    for (bytes, unit) in payload[prefix..]
        .chunks_exact(size_of::<u16>())
        .zip(text.encode_utf16())
    {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, size_of::<u16>() as u64)?;
        let encoded = unit.to_le_bytes();
        if bytes != encoded || escaped && encoded[0] == 0 {
            return Err(QueryCreationError::Preservation);
        }
    }
    Ok(())
}

fn capture(
    image: &NativeImage,
    number: u32,
    owner: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Page, QueryCreationError> {
    Page::capture_retained(page(image, number)?, owner, budget, cancelled).map_err(Into::into)
}

fn page(image: &NativeImage, number: u32) -> Result<&[u8], QueryCreationError> {
    let start = usize::try_from(number)
        .ok()
        .and_then(|number| number.checked_mul(PAGE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    image
        .as_bytes()
        .get(start..end)
        .ok_or(QueryCreationError::Preservation)
}

fn record<'a>(
    image: &'a NativeImage,
    number: u32,
    range: &Range<usize>,
) -> Result<&'a [u8], QueryCreationError> {
    page(image, number)?
        .get(range.start..range.end)
        .ok_or(QueryCreationError::Preservation)
}

fn append_scope(offset: usize, before: &Page, after: &Page) -> bool {
    (2..4).contains(&offset)
        || (12..14).contains(&offset)
        || (slot_offset(before.count)..slot_offset(after.count)).contains(&offset)
        || (after.start..before.start).contains(&offset)
}
