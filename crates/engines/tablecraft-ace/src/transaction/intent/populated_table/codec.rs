//! u64 complete framing and row-major closed scalar carriers; no SQL replay.
use super::super::{
    binding,
    first_table::{
        MAX_NATIVE_DECLARATION_TEMPLATE_BYTES, decode_native_declaration_template,
        encode_declaration_template,
    },
    row_carrier::{self as carrier, Borrowed, Profile},
};
use super::{NativePopulatedAddTableIntent, PopulatedTableIntentError};
use crate::{
    CreateColumn, FirstTableDefinition, InspectError, format::data_type::MAX_SHORT_TEXT_UNITS,
    header::check, limits::MAX_USER_COLUMNS,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const MAGIC: &[u8; 8] = b"TCPT0001";
const PREFIX: &[u8; 4] = b"TCPT";
const NESTED_MAGIC: &[u8; 8] = b"TCNT0007";
const TOTAL_AT: usize = MAGIC.len();
const PROFILE_AT: usize = TOTAL_AT + size_of::<u64>();
const FLAGS_AT: usize = PROFILE_AT + size_of::<u16>();
const RESERVED_AT: usize = FLAGS_AT + size_of::<u16>();
const BEFORE_AT: usize = RESERVED_AT + size_of::<u32>();
const BINDING_AT: usize = BEFORE_AT + size_of::<u64>();
const NESTED_LENGTH_AT: usize = BINDING_AT + 32;
const ROWS_AT: usize = NESTED_LENGTH_AT + size_of::<u32>();
const COLUMNS_AT: usize = ROWS_AT + size_of::<u32>();
const TAIL_RESERVED_AT: usize = COLUMNS_AT + size_of::<u16>();
const HEADER_BYTES: usize = TAIL_RESERVED_AT + 6;
const CELL_LENGTH_AT: usize = size_of::<u32>();
const PROFILE: u16 = 1;
// This construction language has Short Text/Binary, not Memo/OLE fields. Bound
// raw scalar scanning by the largest possible native declaration before UTF8
// validation; the actual selected field's UTF16 width is checked separately.
const MAX_SCALAR_PAYLOAD_BYTES: usize = MAX_SHORT_TEXT_UNITS * char::MAX.len_utf8();

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], PopulatedTableIntentError> {
    bytes
        .try_into()
        .map_err(|_| PopulatedTableIntentError::InvalidRecord)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], PopulatedTableIntentError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(PopulatedTableIntentError::InvalidRecord)?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or(PopulatedTableIntentError::InvalidRecord)?;
        self.at = end;
        Ok(bytes)
    }

    fn cell(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Borrowed<'a>, PopulatedTableIntentError> {
        check(budget, cancelled)?;
        let header = self.take(carrier::HEADER_BYTES)?;
        if header[1..CELL_LENGTH_AT] != [0; 3] {
            return Err(PopulatedTableIntentError::InvalidRecord);
        }
        let length =
            u32::from_le_bytes(array(&header[CELL_LENGTH_AT..carrier::HEADER_BYTES])?) as usize;
        if length > MAX_SCALAR_PAYLOAD_BYTES {
            return Err(PopulatedTableIntentError::InvalidRecord);
        }
        let bytes = self.take(length)?;
        Ok(carrier::length_prefixed_row_parse(
            header[0],
            bytes,
            Profile::Wider,
            budget,
            cancelled,
        )?)
    }

    fn finish(self) -> Result<(), PopulatedTableIntentError> {
        if self.at != self.bytes.len() {
            return Err(PopulatedTableIntentError::InvalidRecord);
        }
        Ok(())
    }
}

fn columns<'a>(
    definition: &'a FirstTableDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CreateColumn<'a>>, PopulatedTableIntentError> {
    if !definition.is_native() || !definition.is_manual() || definition.primary_position().is_some()
    {
        return Err(PopulatedTableIntentError::UnsupportedProfile);
    }
    let columns = definition
        .borrowed_columns(budget, cancelled)
        .map_err(super::FirstTableIntentError::from)?;
    for column in &columns {
        check(budget, cancelled)?;
        if column.incrementing_autonumber()
            || column.primary_key_name().is_some()
            || column.required().is_some()
            || column.default_literal().is_some()
            || column.allow_zero_length().is_some()
            || column.description().is_some()
            || column.unicode_compression()
        {
            return Err(PopulatedTableIntentError::UnsupportedProfile);
        }
    }
    Ok(columns)
}

fn validate_cell(
    value: &Borrowed<'_>,
    column: CreateColumn<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PopulatedTableIntentError> {
    check(budget, cancelled)?;
    match value {
        Borrowed::Scalar(Value::Null) if column.scalar_type() != ScalarType::Boolean => Ok(()),
        Borrowed::Scalar(value) if ScalarType::of(value) == Some(column.scalar_type()) => {
            if let Value::Decimal(value) = value {
                column
                    .decimal_constraints()
                    .ok_or(PopulatedTableIntentError::InvalidRecord)?
                    .validate(*value)
                    .map_err(|_| PopulatedTableIntentError::InvalidRecord)?;
            }
            Ok(())
        }
        Borrowed::Text(text) if column.scalar_type() == ScalarType::Text => {
            let maximum = column
                .max_text_utf16_units()
                .ok_or(PopulatedTableIntentError::InvalidRecord)?;
            let mut units = 0;
            for character in text.chars() {
                check(budget, cancelled)?;
                units += character.len_utf16();
                if units > maximum {
                    return Err(PopulatedTableIntentError::InvalidRecord);
                }
            }
            Ok(())
        }
        Borrowed::Binary(bytes)
            if column.scalar_type() == ScalarType::Binary
                && column
                    .max_binary_bytes()
                    .is_some_and(|maximum| bytes.len() <= maximum) =>
        {
            Ok(())
        }
        _ => Err(PopulatedTableIntentError::InvalidRecord),
    }
}

pub(super) fn validate_request(
    definition: &FirstTableDefinition,
    rows: &[Vec<Value>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), PopulatedTableIntentError> {
    check(budget, cancelled)?;
    u32::try_from(rows.len()).map_err(|_| PopulatedTableIntentError::InvalidRecord)?;
    let columns = columns(definition, budget, cancelled)?;
    for row in rows {
        check(budget, cancelled)?;
        if row.len() != columns.len() {
            return Err(PopulatedTableIntentError::InvalidRecord);
        }
        for (value, &column) in row.iter().zip(&columns) {
            let value = carrier::length_prefixed_row_view(value, Profile::Wider)?;
            if value.bytes().len() > MAX_SCALAR_PAYLOAD_BYTES {
                return Err(PopulatedTableIntentError::InvalidRecord);
            }
            let borrowed = carrier::length_prefixed_row_parse(
                value.tag(),
                value.bytes(),
                Profile::Wider,
                budget,
                cancelled,
            )?;
            validate_cell(&borrowed, column, budget, cancelled)?;
        }
    }
    Ok(())
}

fn append(
    bytes: &mut Vec<u8>,
    value: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    for chunk in value.chunks(carrier::COPY_BYTES) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, chunk.len() as u64)?;
        bytes.extend_from_slice(chunk);
    }
    Ok(())
}

pub(super) fn encode(
    definition: &FirstTableDefinition,
    rows: &[Vec<Value>],
    before_length: u64,
    before_binding: [u8; 32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, PopulatedTableIntentError> {
    validate_request(definition, rows, budget, cancelled)?;
    binding::retained_length(before_length)?;
    let nested = encode_declaration_template(definition, budget, cancelled)?;
    if !nested.starts_with(NESTED_MAGIC) {
        return Err(PopulatedTableIntentError::UnsupportedProfile);
    }
    let nested_length =
        u32::try_from(nested.len()).map_err(|_| PopulatedTableIntentError::InvalidRecord)?;
    let mut total = HEADER_BYTES
        .checked_add(nested.len())
        .ok_or(PopulatedTableIntentError::InvalidRecord)?;
    for row in rows {
        for value in row {
            check(budget, cancelled)?;
            let encoded = carrier::length_prefixed_row_view(value, Profile::Wider)?;
            u32::try_from(encoded.bytes().len())
                .map_err(|_| PopulatedTableIntentError::InvalidRecord)?;
            total = total
                .checked_add(carrier::HEADER_BYTES)
                .and_then(|length| length.checked_add(encoded.bytes().len()))
                .ok_or(PopulatedTableIntentError::InvalidRecord)?;
        }
    }
    let total_wire = u64::try_from(total).map_err(|_| PopulatedTableIntentError::InvalidRecord)?;
    budget
        .charge_allocation(total_wire, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    let mut header = [0; HEADER_BYTES];
    header[..TOTAL_AT].copy_from_slice(MAGIC);
    header[TOTAL_AT..PROFILE_AT].copy_from_slice(&total_wire.to_le_bytes());
    header[PROFILE_AT..FLAGS_AT].copy_from_slice(&PROFILE.to_le_bytes());
    header[BEFORE_AT..BINDING_AT].copy_from_slice(&before_length.to_le_bytes());
    header[BINDING_AT..NESTED_LENGTH_AT].copy_from_slice(&before_binding);
    header[NESTED_LENGTH_AT..ROWS_AT].copy_from_slice(&nested_length.to_le_bytes());
    header[ROWS_AT..COLUMNS_AT].copy_from_slice(&(rows.len() as u32).to_le_bytes());
    header[COLUMNS_AT..TAIL_RESERVED_AT]
        .copy_from_slice(&(definition.column_count() as u16).to_le_bytes());
    append(&mut bytes, &header, budget, cancelled)?;
    append(&mut bytes, &nested, budget, cancelled)?;
    for row in rows {
        for value in row {
            let encoded = carrier::length_prefixed_row_view(value, Profile::Wider)?;
            let mut header = [0; carrier::HEADER_BYTES];
            header[0] = encoded.tag();
            header[CELL_LENGTH_AT..carrier::HEADER_BYTES]
                .copy_from_slice(&(encoded.bytes().len() as u32).to_le_bytes());
            append(&mut bytes, &header, budget, cancelled)?;
            append(&mut bytes, encoded.bytes(), budget, cancelled)?;
        }
    }
    check(budget, cancelled)?;
    if bytes.len() != total {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    Ok(bytes)
}

pub(super) fn decode(
    raw: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativePopulatedAddTableIntent, PopulatedTableIntentError> {
    check(budget, cancelled)?;
    if raw.len() < HEADER_BYTES || !raw.starts_with(PREFIX) {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    if !raw.starts_with(MAGIC) {
        return Err(PopulatedTableIntentError::UnsupportedVersion);
    }
    let total = u64::from_le_bytes(array(&raw[TOTAL_AT..PROFILE_AT])?);
    if usize::try_from(total).ok() != Some(raw.len()) {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    if u16::from_le_bytes(array(&raw[PROFILE_AT..FLAGS_AT])?) != PROFILE {
        return Err(PopulatedTableIntentError::UnsupportedProfile);
    }
    if raw[FLAGS_AT..BEFORE_AT] != [0; 6] || raw[TAIL_RESERVED_AT..HEADER_BYTES] != [0; 6] {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(array(&raw[BEFORE_AT..BINDING_AT])?);
    binding::retained_length(before_length)?;
    let before_binding = array(&raw[BINDING_AT..NESTED_LENGTH_AT])?;
    let nested_length = u32::from_le_bytes(array(&raw[NESTED_LENGTH_AT..ROWS_AT])?) as usize;
    let count = u32::from_le_bytes(array(&raw[ROWS_AT..COLUMNS_AT])?) as usize;
    let width = usize::from(u16::from_le_bytes(array(
        &raw[COLUMNS_AT..TAIL_RESERVED_AT],
    )?));
    if width == 0
        || width > MAX_USER_COLUMNS
        || nested_length > MAX_NATIVE_DECLARATION_TEMPLATE_BYTES
    {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    let mut stream = Cursor {
        bytes: raw,
        at: HEADER_BYTES,
    };
    let nested = stream.take(nested_length)?;
    if !nested.starts_with(NESTED_MAGIC) {
        return Err(PopulatedTableIntentError::UnsupportedVersion);
    }
    let cells = count
        .checked_mul(width)
        .ok_or(PopulatedTableIntentError::InvalidRecord)?;
    if cells
        .checked_mul(carrier::HEADER_BYTES)
        .is_none_or(|minimum| minimum > raw.len() - stream.at)
    {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    let rows_start = stream.at;
    // Every frame and scalar constructor is checked before any declaration or
    // owned-row allocation. No failed prefix can become a materialized request.
    for _ in 0..cells {
        stream.cell(budget, cancelled)?;
    }
    stream.finish()?;
    let definition = decode_native_declaration_template(nested, budget, cancelled)?;
    let columns = columns(&definition, budget, cancelled)?;
    if columns.len() != width {
        return Err(PopulatedTableIntentError::InvalidRecord);
    }
    let mut stream = Cursor {
        bytes: raw,
        at: rows_start,
    };
    // Bounded declaration ownership precedes cross-checking every borrowed cell
    // against its real native width/p/s, still before any owned-row allocation.
    for _ in 0..count {
        for &column in &columns {
            validate_cell(&stream.cell(budget, cancelled)?, column, budget, cancelled)?;
        }
    }
    stream.finish()?;
    drop(columns);
    budget
        .charge_allocation(count as u64, size_of::<Vec<Value>>() as u64)
        .map_err(InspectError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut stream = Cursor {
        bytes: raw,
        at: rows_start,
    };
    for _ in 0..count {
        check(budget, cancelled)?;
        budget
            .charge_allocation(width as u64, size_of::<Value>() as u64)
            .map_err(InspectError::from)?;
        let mut row = Vec::new();
        row.try_reserve_exact(width)
            .map_err(|_| InspectError::Allocation)?;
        for _ in 0..width {
            row.push(carrier::owned(
                stream.cell(budget, cancelled)?,
                budget,
                cancelled,
            )?);
        }
        rows.push(row);
    }
    stream.finish()?;
    check(budget, cancelled)?;
    Ok(NativePopulatedAddTableIntent {
        definition,
        rows,
        before_length,
        before_binding,
    })
}
