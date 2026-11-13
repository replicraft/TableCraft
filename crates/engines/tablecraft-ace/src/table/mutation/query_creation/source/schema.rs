//! Exact complete system declarations for the first append-only QueryDef profile.
use crate::{
    NativeImage, NativeTableStorage, QueryCreationError, TableSchema,
    format::{data_type, row::COLUMN_COUNT_BYTES},
    header::check,
    inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;

const SYSTEM_FLAG: u8 = 0x10;
const OBJECT_FLAGS: u32 = 0x8000_0002;
// Native17 catalog Owner has this observed descriptor bit in addition to the
// ordinary variable/system flags. Its value is required NULL by this source.
const OWNER_FLAGS: u8 = 0x22;

pub(super) fn capture(
    image: &NativeImage,
    name: &str,
    parent: i32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSchema, QueryCreationError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        name,
        budget,
        cancelled,
    )?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == schema.object_id())
        .ok_or(QueryCreationError::Source)?;
    if origin.table_storage() != Some(NativeTableStorage::Local)
        || origin.type_code() != 1
        || origin.parent_id() != parent
        || !origin.is_system()
        || origin.flags() & !OBJECT_FLAGS != 0
        || u32::try_from(origin.native_id()).ok() != Some(schema.definition_page())
        || schema.layout.physical_indexes != 0
        || schema.layout.logical_indexes != 0
        || schema.layout.max_columns != schema.columns().len()
    {
        return Err(QueryCreationError::Source);
    }
    let expected: &[(&str, u8, bool)] = if name == "MSysObjects" {
        &[
            ("Id", data_type::INT32, true),
            ("ParentId", data_type::INT32, true),
            ("Name", data_type::SHORT_TEXT, false),
            ("Type", data_type::INT16, true),
            ("DateCreate", data_type::DATE_TIME, true),
            ("DateUpdate", data_type::DATE_TIME, true),
            ("Owner", data_type::SHORT_BINARY, false),
            ("Flags", data_type::INT32, true),
            ("Database", data_type::LONG_TEXT, false),
            ("Connect", data_type::LONG_TEXT, false),
            ("ForeignName", data_type::SHORT_TEXT, false),
            ("RmtInfoShort", data_type::SHORT_BINARY, false),
            ("RmtInfoLong", data_type::OLE, false),
            ("Lv", data_type::OLE, false),
            ("LvProp", data_type::OLE, false),
            ("LvModule", data_type::OLE, false),
            ("LvExtra", data_type::OLE, false),
        ]
    } else {
        &[
            ("ObjectId", data_type::INT32, true),
            ("Attribute", data_type::BYTE, true),
            ("Order", data_type::SHORT_BINARY, false),
            ("Name1", data_type::SHORT_TEXT, false),
            ("Name2", data_type::SHORT_TEXT, false),
            ("Expression", data_type::LONG_TEXT, false),
            ("Flag", data_type::INT16, true),
            ("LvExtra", data_type::INT32, true),
        ]
    };
    if schema.columns().len() != expected.len() {
        return Err(QueryCreationError::Source);
    }
    let mut variable = 0;
    for (position, &(name, kind, fixed)) in expected.iter().enumerate() {
        check(budget, cancelled)?;
        let column = &schema.columns()[position];
        let width = match kind {
            data_type::BYTE => Some(data_type::BYTE_BYTES),
            data_type::INT16 => Some(data_type::INT16_BYTES),
            data_type::INT32 => Some(data_type::INT32_BYTES),
            data_type::DATE_TIME => Some(data_type::DATE_TIME_BYTES),
            _ => None,
        };
        if width.is_some_and(|width| column.storage_length() != width) {
            return Err(QueryCreationError::Source);
        }
        if !fixed {
            if column.field.variable != variable {
                return Err(QueryCreationError::Source);
            }
            variable += 1;
        }
        let flags = if name == "Owner" && schema.name() == "MSysObjects" {
            OWNER_FLAGS
        } else if fixed {
            3
        } else {
            2
        };
        if column.name() != name
            || column.native_type() != kind
            || column.fixed() != fixed
            || column.number() != position
            || column.flags() & !SYSTEM_FLAG != flags
            || column.extended_flags() != 0
        {
            return Err(QueryCreationError::Source);
        }
    }
    if variable != schema.layout.variable_slots {
        return Err(QueryCreationError::Source);
    }
    fixed_spans(&schema, budget, cancelled)?;
    Ok(schema)
}

/// Layout decoding supplies each span and its maximum end, but it does not
/// prove that simultaneously active fixed fields occupy distinct bytes.
fn fixed_spans(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryCreationError> {
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if !column.fixed() {
            continue;
        }
        let start = COLUMN_COUNT_BYTES
            .checked_add(column.field.offset)
            .ok_or(QueryCreationError::Source)?;
        let end = start
            .checked_add(column.storage_length())
            .filter(|end| *end <= schema.layout.fixed_end)
            .ok_or(QueryCreationError::Source)?;
        for previous in &schema.columns()[..position] {
            check(budget, cancelled)?;
            if !previous.fixed() {
                continue;
            }
            let previous_start = COLUMN_COUNT_BYTES
                .checked_add(previous.field.offset)
                .ok_or(QueryCreationError::Source)?;
            let previous_end = previous_start
                .checked_add(previous.storage_length())
                .filter(|end| *end <= schema.layout.fixed_end)
                .ok_or(QueryCreationError::Source)?;
            if start < previous_end && previous_start < end {
                return Err(QueryCreationError::Source);
            }
        }
    }
    Ok(())
}
