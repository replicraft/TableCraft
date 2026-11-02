//! Explicit creation field policies/defaults, sized before image allocation.
use super::{
    CreateDatabaseError,
    defaults::Literal,
    input::{Cell, Columns, Rows},
};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{FieldConstraints, ScalarType};

const SIGNATURE: &[u8; 4] = b"MR2\0";
const NAME_LIST: u16 = 0x80;
const COLUMN_VALUES: u16 = 1;
const PROPERTY_BOOLEAN: u8 = 1;
const PROPERTY_MEMO: u8 = 12;
const PROPERTY_DDL: u8 = 1;
const BLOCK_HEADER: usize = size_of::<u32>() + size_of::<u16>();
const GROUP_NAME_HEADER: usize = size_of::<u32>();
const BOOLEAN_ENTRY: usize = 3 * size_of::<u16>() + 3 * size_of::<u8>();
const VALUE_HEADER: usize = 3 * size_of::<u16>() + 2 * size_of::<u8>();
const INLINE_HEADER: usize = 3 * size_of::<u32>();
const INLINE_FLAG: u32 = 0x8000_0000;
const REQUIRED: &str = "Required";
const ALLOW_EMPTY: &str = "AllowZeroLength";
const DEFAULT_VALUE: &str = "DefaultValue";

pub(super) struct Prepared<'a> {
    columns: Columns<'a>,
    length: usize,
    required: bool,
    allow_empty: bool,
    default_value: bool,
}

fn add(length: usize, extra: usize) -> Result<usize, CreateDatabaseError> {
    length
        .checked_add(extra)
        .ok_or(CreateDatabaseError::PageCapacity)
}

fn name_length(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, CreateDatabaseError> {
    let mut bytes = size_of::<u16>();
    for _ in name.encode_utf16() {
        check(budget, cancelled)?;
        bytes = add(bytes, size_of::<u16>())?;
    }
    Ok(bytes)
}

pub(super) fn capture<'a>(
    columns: Columns<'a>,
    rows: Rows<'a>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Prepared<'a>>, CreateDatabaseError> {
    let mut required = false;
    let mut allow_empty = false;
    let mut default_value = false;
    let mut groups = 0;
    let mut length = SIGNATURE.len() + BLOCK_HEADER;
    for position in 0..columns.count() {
        check(budget, cancelled)?;
        let (field_required, field_empty) = columns.policies(position);
        if field_empty.is_some() && columns.scalar(position) != ScalarType::Text {
            return Err(CreateDatabaseError::Declaration);
        }
        let default = columns.default_literal(position);
        let literal = default
            .map(|value| Literal::capture(value, budget, cancelled))
            .transpose()?;
        let boolean_entries =
            usize::from(field_required.is_some()) + usize::from(field_empty.is_some());
        if boolean_entries == 0 && literal.is_none() {
            continue;
        }
        groups += 1;
        required |= field_required.is_some();
        allow_empty |= field_empty.is_some();
        default_value |= literal.is_some();
        let label = name_length(columns.name(position), budget, cancelled)?;
        length = add(length, BLOCK_HEADER + GROUP_NAME_HEADER)?;
        length = add(length, label)?;
        length = add(length, boolean_entries * BOOLEAN_ENTRY)?;
        if let Some(literal) = literal {
            length = add(length, VALUE_HEADER + literal.payload_bytes())?;
        }
        for row in 0..rows.count() {
            check(budget, cancelled)?;
            let Cell::Value(value) = rows.cell(row, position) else {
                return Err(CreateDatabaseError::Layout);
            };
            if let Some(value_required) = field_required {
                FieldConstraints::new(value_required, true).check(value)?;
            }
            if let Some(value_empty) = field_empty {
                FieldConstraints::new(false, value_empty).check(value)?;
            }
        }
    }
    if groups == 0 {
        return Ok(None);
    }
    if required {
        length = add(length, name_length(REQUIRED, budget, cancelled)?)?;
    }
    if allow_empty {
        length = add(length, name_length(ALLOW_EMPTY, budget, cancelled)?)?;
    }
    if default_value {
        length = add(length, name_length(DEFAULT_VALUE, budget, cancelled)?)?;
    }
    // A whole catalog page is a stricter bound than the format's integer widths.
    if add(length, INLINE_HEADER)? > crate::format::page::PAGE_BYTES {
        return Err(CreateDatabaseError::PageCapacity);
    }
    check(budget, cancelled)?;
    Ok(Some(Prepared {
        columns,
        length,
        required,
        allow_empty,
        default_value,
    }))
}

struct Writer<'a, 'budget> {
    bytes: &'a mut [u8],
    at: usize,
    budget: &'budget mut ResourceBudget,
    cancelled: &'budget dyn Fn() -> bool,
}

impl Writer<'_, '_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), CreateDatabaseError> {
        check(self.budget, self.cancelled)?;
        let end = self
            .at
            .checked_add(bytes.len())
            .ok_or(CreateDatabaseError::Layout)?;
        let target = self
            .bytes
            .get_mut(self.at..end)
            .ok_or(CreateDatabaseError::Layout)?;
        self.budget
            .charge(Resource::Steps, bytes.len() as u64)
            .map_err(InspectError::from)?;
        target.copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }

    fn short(&mut self, value: usize) -> Result<(), CreateDatabaseError> {
        let value = u16::try_from(value).map_err(|_| CreateDatabaseError::PageCapacity)?;
        self.put(&value.to_le_bytes())
    }

    fn long(&mut self, value: usize) -> Result<(), CreateDatabaseError> {
        let value = u32::try_from(value).map_err(|_| CreateDatabaseError::PageCapacity)?;
        self.put(&value.to_le_bytes())
    }

    fn name(&mut self, name: &str, length: usize) -> Result<(), CreateDatabaseError> {
        self.short(length - size_of::<u16>())?;
        for unit in name.encode_utf16() {
            self.put(&unit.to_le_bytes())?;
        }
        Ok(())
    }

    fn default(&mut self, index: usize, literal: &Literal) -> Result<(), CreateDatabaseError> {
        self.short(VALUE_HEADER + literal.payload_bytes())?;
        self.put(&[PROPERTY_DDL, PROPERTY_MEMO])?;
        self.short(index)?;
        self.short(literal.payload_bytes())?;
        for unit in literal.units() {
            self.put(&unit.to_le_bytes())?;
        }
        Ok(())
    }

    fn boolean(&mut self, index: usize, value: bool) -> Result<(), CreateDatabaseError> {
        self.short(BOOLEAN_ENTRY)?;
        self.put(&[PROPERTY_DDL, PROPERTY_BOOLEAN])?;
        self.short(index)?;
        self.short(size_of::<u8>())?;
        self.put(&[if value { u8::MAX } else { 0 }])
    }
}

impl Prepared<'_> {
    pub fn inline_length(&self) -> usize {
        INLINE_HEADER + self.length
    }

    pub fn write_inline(
        &self,
        bytes: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        if bytes.len() != self.inline_length() {
            return Err(CreateDatabaseError::Layout);
        }
        let required_length = if self.required {
            name_length(REQUIRED, budget, cancelled)?
        } else {
            0
        };
        let empty_length = if self.allow_empty {
            name_length(ALLOW_EMPTY, budget, cancelled)?
        } else {
            0
        };
        let default_length = if self.default_value {
            name_length(DEFAULT_VALUE, budget, cancelled)?
        } else {
            0
        };
        let mut writer = Writer {
            bytes,
            at: 0,
            budget,
            cancelled,
        };
        writer.put(&(INLINE_FLAG | self.length as u32).to_le_bytes())?;
        writer.put(&[0; 2 * size_of::<u32>()])?;
        writer.put(SIGNATURE)?;
        writer.long(BLOCK_HEADER + required_length + empty_length + default_length)?;
        writer.put(&NAME_LIST.to_le_bytes())?;
        if self.required {
            writer.name(REQUIRED, required_length)?;
        }
        if self.allow_empty {
            writer.name(ALLOW_EMPTY, empty_length)?;
        }
        if self.default_value {
            writer.name(DEFAULT_VALUE, default_length)?;
        }
        for position in 0..self.columns.count() {
            check(writer.budget, cancelled)?;
            let (required, empty) = self.columns.policies(position);
            let boolean_count = usize::from(required.is_some()) + usize::from(empty.is_some());
            let literal = self
                .columns
                .default_literal(position)
                .map(|value| Literal::capture(value, writer.budget, cancelled))
                .transpose()?;
            if boolean_count == 0 && literal.is_none() {
                continue;
            }
            let name = self.columns.name(position);
            let label = name_length(name, writer.budget, cancelled)?;
            let group_name = GROUP_NAME_HEADER + label;
            let default_bytes = literal
                .as_ref()
                .map_or(0, |value| VALUE_HEADER + value.payload_bytes());
            writer
                .long(BLOCK_HEADER + group_name + boolean_count * BOOLEAN_ENTRY + default_bytes)?;
            writer.put(&COLUMN_VALUES.to_le_bytes())?;
            writer.long(group_name)?;
            writer.name(name, label)?;
            if let Some(value) = required {
                writer.boolean(0, value)?;
            }
            if let Some(value) = empty {
                writer.boolean(usize::from(self.required), value)?;
            }
            if let Some(literal) = literal {
                let index = usize::from(self.required) + usize::from(self.allow_empty);
                writer.default(index, &literal)?;
            }
        }
        if writer.at != writer.bytes.len() {
            return Err(CreateDatabaseError::Layout);
        }
        check(writer.budget, cancelled)?;
        Ok(())
    }
}
