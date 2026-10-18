use crate::InspectError;
use crate::format::{
    bytes::{u16_at, u32_at},
    layout::{Field, Layout},
    text,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::ObjectKind;

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
    let mask = count.div_ceil(8);
    let count_pos = bytes
        .len()
        .checked_sub(mask + 2)
        .ok_or(InspectError::Truncated)?;
    let variables = usize::from(u16_at(bytes, count_pos)?);
    if variables == 0 || variables > layout.variable_slots {
        return Err(InspectError::Unsupported);
    }
    let table_start = count_pos
        .checked_sub(2 * (variables + 1))
        .ok_or(InspectError::Truncated)?;
    let offset = |index: usize| -> Result<usize, InspectError> {
        Ok(usize::from(u16_at(
            bytes,
            count_pos
                .checked_sub(2 * (index + 1))
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
            || bytes[bytes.len() - mask + field.number / 8] & (1 << (field.number % 8)) == 0
        {
            return Err(InspectError::Malformed);
        }
        Ok(())
    };
    let integer = |name: &str| -> Result<i32, InspectError> {
        let field = layout.field(name)?;
        present(field)?;
        if 2 + field.offset + 4 > table_start {
            return Err(InspectError::Malformed);
        }
        Ok(u32_at(bytes, 2 + field.offset)? as i32)
    };
    let kind = layout.field("Type")?;
    present(kind)?;
    if 2 + kind.offset + 2 > table_start {
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
        kind: u16_at(bytes, 2 + kind.offset)? as i16,
        flags: integer("Flags")? as u32,
        name,
    })
}
pub(super) fn kind(code: i16) -> ObjectKind {
    match code {
        1 | 4 | 6 => ObjectKind::Table,
        5 => ObjectKind::Query,
        3 => ObjectKind::Container,
        8 => ObjectKind::Relationship,
        -32768 => ObjectKind::Form,
        -32764 => ObjectKind::Report,
        -32766 => ObjectKind::Macro,
        -32761 => ObjectKind::Module,
        _ => ObjectKind::Unknown,
    }
}
