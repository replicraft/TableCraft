//! Borrowed declared cells below the operation-specific system-row certificate.
use super::super::{
    record_insertion::{build::assemble_with, fixed},
    record_serialization::{
        copy,
        long_value::{Encoded, StorageChoice},
        storage, utf16,
    },
};
use crate::{
    EditError, InspectError, QueryAttribute, QueryCreationDefinition, QueryCreationError,
    TableSchema,
    format::data_type::{LONG_TEXT, SHORT_BINARY, SHORT_TEXT},
    header::check,
};
use arrayvec::ArrayString;
use std::fmt::Write;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{LegacyDateTime, Value},
};

const INTEGER_TEXT_BYTES: usize = i32::MIN.unsigned_abs().ilog10() as usize + 2;
const SELECT_TYPE: i16 = 1;
const QUERY_OBJECT_TYPE: i16 = 5;
const ORDER: [u8; size_of::<u32>()] = [0, 0, 0, 1];
const ATTRIBUTES: [QueryAttribute; 5] = [
    QueryAttribute::Start,
    QueryAttribute::Type,
    QueryAttribute::Flags,
    QueryAttribute::Column,
    QueryAttribute::End,
];

#[derive(Clone, Copy)]
enum Cell<'a> {
    Null,
    Byte(u8),
    Short(i16),
    Long(i32),
    Date(LegacyDateTime),
    Text(&'a str),
    Binary(&'a [u8]),
}

impl Cell<'_> {
    fn present(self) -> bool {
        !matches!(self, Self::Null)
    }

    fn scalar(self) -> Result<Value, EditError> {
        Ok(match self {
            Self::Null => Value::Null,
            Self::Byte(value) => Value::Byte(value),
            Self::Short(value) => Value::Int16(value),
            Self::Long(value) => Value::Int32(value),
            Self::Date(value) => Value::DateTime(value),
            Self::Text(_) | Self::Binary(_) => return Err(EditError::Preservation),
        })
    }
}

pub(super) fn catalog(
    schema: &TableSchema,
    parent: i32,
    native_id: i32,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, QueryCreationError> {
    encode(
        schema,
        |name| match name {
            "Id" => Cell::Long(native_id),
            "ParentId" => Cell::Long(parent),
            "Type" => Cell::Short(QUERY_OBJECT_TYPE),
            "Flags" => Cell::Long(0),
            "DateCreate" | "DateUpdate" => Cell::Date(definition.captured_at()),
            "Name" => Cell::Text(definition.name()),
            _ => Cell::Null,
        },
        budget,
        cancelled,
    )
}

pub(super) fn fragments(
    schema: &TableSchema,
    native_id: i32,
    definition: &QueryCreationDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, QueryCreationError> {
    check(budget, cancelled)?;
    budget.charge_allocation(ATTRIBUTES.len() as u64, size_of::<Vec<u8>>() as u64)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(ATTRIBUTES.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut literal = ArrayString::<INTEGER_TEXT_BYTES>::new();
    write!(literal, "{}", definition.value()).map_err(|_| QueryCreationError::Definition)?;

    for attribute in ATTRIBUTES {
        records.push(encode(
            schema,
            |name| match (name, attribute) {
                ("ObjectId", _) => Cell::Long(native_id),
                ("Attribute", _) => Cell::Byte(attribute.code()),
                ("Order", _) => Cell::Binary(&ORDER),
                ("Flag", QueryAttribute::Type) => Cell::Short(SELECT_TYPE),
                ("Flag", QueryAttribute::Flags) => Cell::Short(0),
                ("Expression", QueryAttribute::Column) => Cell::Text(&literal),
                ("Name1", QueryAttribute::Column) => Cell::Text(definition.alias()),
                _ => Cell::Null,
            },
            budget,
            cancelled,
        )?);
    }
    Ok(records)
}

fn encode<'cell>(
    schema: &TableSchema,
    cell: impl Fn(&str) -> Cell<'cell>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, QueryCreationError> {
    check(budget, cancelled)?;
    let count = schema.layout.variable_slots;
    budget.charge_allocation(count as u64, size_of::<usize>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<Vec<u8>>() as u64)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    variables.resize(count, usize::MAX);
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if !column.fixed() {
            let slot = variables
                .get_mut(column.field.variable)
                .filter(|slot| **slot == usize::MAX)
                .ok_or(QueryCreationError::Source)?;
            *slot = position;
        }
    }
    for &position in &variables {
        let column = schema
            .columns()
            .get(position)
            .ok_or(QueryCreationError::Source)?;
        let payload = match cell(column.name()) {
            Cell::Null => Vec::new(),
            Cell::Binary(bytes) if column.native_type() == SHORT_BINARY => {
                if bytes.len() > column.storage_length() {
                    return Err(QueryCreationError::Definition);
                }
                copy(bytes, budget, cancelled)?
            }
            Cell::Text(text) if column.native_type() == SHORT_TEXT => {
                storage::encode_text_preferred(column, text, budget, cancelled)?
            }
            Cell::Text(text) if column.native_type() == LONG_TEXT => {
                let encoded = Encoded::Present(utf16::encode(text, budget, cancelled)?);
                if encoded.preferred() != StorageChoice::Inline {
                    return Err(QueryCreationError::Definition);
                }
                encoded.descriptor(StorageChoice::Inline, None, budget, cancelled)?
            }
            _ => return Err(QueryCreationError::Definition),
        };
        payloads.push(payload);
    }

    assemble_with(
        schema,
        &variables,
        payloads,
        |_, column, target| {
            let value = cell(column.name());
            if column.fixed() {
                fixed::write(column, &value.scalar()?, target)
            } else {
                Ok(value.present())
            }
        },
        budget,
        cancelled,
    )
    .map_err(Into::into)
}
