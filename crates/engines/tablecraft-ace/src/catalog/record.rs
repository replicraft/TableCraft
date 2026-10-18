use crate::InspectError;
use crate::format::row::{
    COLUMN_COUNT_BYTES, MASK_BITS, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
};
use crate::format::{
    bytes::{u16_at, u32_at},
    layout::{Field, Layout},
    text,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::ObjectKind;

const LOCAL_TABLE: i16 = 1;
const ODBC_TABLE: i16 = 4;
const LINKED_TABLE: i16 = 6;
const QUERY: i16 = 5;
const CONTAINER: i16 = 3;
const RELATIONSHIP: i16 = 8;
const FORM: i16 = -32768;
const REPORT: i16 = -32764;
const MACRO: i16 = -32766;
const MODULE: i16 = -32761;

pub(super) struct Record {
    pub id: i32,
    pub parent: i32,
    pub kind: i16,
    pub flags: u32,
    pub name: String,
}
pub(super) fn decode(
    bytes: &[u8],
    layout: &Layout,
    budget: &mut ResourceBudget,
) -> Result<Record, InspectError> {
    let count = usize::from(u16_at(bytes, 0)?);
    if count > layout.max_columns {
        return Err(InspectError::Malformed);
    }
    let mask = count.div_ceil(MASK_BITS);
    let count_pos = bytes
        .len()
        .checked_sub(mask + VARIABLE_COUNT_BYTES)
        .ok_or(InspectError::Truncated)?;
    let variables = usize::from(u16_at(bytes, count_pos)?);
    if variables == 0 || variables > layout.variable_slots {
        return Err(InspectError::Unsupported);
    }
    let table_start = count_pos
        .checked_sub(VARIABLE_OFFSET_BYTES * (variables + 1))
        .ok_or(InspectError::Truncated)?;
    let offset = |index: usize| -> Result<usize, InspectError> {
        Ok(usize::from(u16_at(
            bytes,
            count_pos
                .checked_sub(VARIABLE_OFFSET_BYTES * (index + 1))
                .ok_or(InspectError::Truncated)?,
        )?))
    };
    let mut previous = layout.fixed_end;
    for i in 0..=variables {
        let next = offset(i)?;
        if next < previous || next > table_start {
            return Err(InspectError::Malformed);
        }
        previous = next;
    }
    let present = |field: &Field| -> Result<(), InspectError> {
        if field.number >= count
            || bytes[bytes.len() - mask + field.number / MASK_BITS]
                & (1 << (field.number % MASK_BITS))
                == 0
        {
            return Err(InspectError::Malformed);
        }
        Ok(())
    };
    let integer = |name: &str| -> Result<i32, InspectError> {
        let field = layout.field(name)?;
        present(field)?;
        if COLUMN_COUNT_BYTES + field.offset + size_of::<i32>() > table_start {
            return Err(InspectError::Malformed);
        }
        Ok(u32_at(bytes, COLUMN_COUNT_BYTES + field.offset)? as i32)
    };
    let kind = layout.field("Type")?;
    present(kind)?;
    if COLUMN_COUNT_BYTES + kind.offset + size_of::<i16>() > table_start {
        return Err(InspectError::Malformed);
    }
    let name = layout.field("Name")?;
    present(name)?;
    if name.variable >= variables {
        return Err(InspectError::Unsupported);
    }
    let encoded_name = &bytes[offset(name.variable)?..offset(name.variable + 1)?];
    if encoded_name.len() > name.length {
        return Err(InspectError::Malformed);
    }
    let name = text::decode(encoded_name, true, budget)?;
    Ok(Record {
        id: integer("Id")?,
        parent: integer("ParentId")?,
        kind: u16_at(bytes, COLUMN_COUNT_BYTES + kind.offset)? as i16,
        flags: integer("Flags")? as u32,
        name,
    })
}
pub(super) fn kind(code: i16) -> ObjectKind {
    match code {
        LOCAL_TABLE | ODBC_TABLE | LINKED_TABLE => ObjectKind::Table,
        QUERY => ObjectKind::Query,
        CONTAINER => ObjectKind::Container,
        RELATIONSHIP => ObjectKind::Relationship,
        FORM => ObjectKind::Form,
        REPORT => ObjectKind::Report,
        MACRO => ObjectKind::Macro,
        MODULE => ObjectKind::Module,
        _ => ObjectKind::Unknown,
    }
}
