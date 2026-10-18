use super::{
    bytes::{u16_at, u32_at},
    reader::Reader,
    text,
};
use crate::InspectError;
use std::io::{Read, Seek};

pub(crate) struct Field {
    pub number: usize,
    pub variable: usize,
    pub fixed: bool,
    pub offset: usize,
    pub length: usize,
    pub kind: u8,
    pub name: String,
    pub flags: u8,
    pub extended_flags: u8,
}
pub(crate) struct Layout {
    pub fields: Vec<Field>,
    pub max_columns: usize,
    pub variable_slots: usize,
    pub fixed_end: usize,
    pub records: u32,
    pub usage: u32,
}
impl Layout {
    pub fn field(&self, name: &str) -> Result<&Field, InspectError> {
        let mut matches = self
            .fields
            .iter()
            .filter(|f| f.name.eq_ignore_ascii_case(name));
        let field = matches.next().ok_or(InspectError::Unsupported)?;
        if matches.next().is_some() {
            return Err(InspectError::Malformed);
        }
        Ok(field)
    }
}
pub(crate) fn read<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    definition: u32,
) -> Result<Layout, InspectError> {
    let page = reader.page(definition)?;
    if page[0..2] != [2, 1] {
        return Err(InspectError::Malformed);
    }
    if u32_at(&page, 4)? != 0 {
        return Err(InspectError::Unsupported);
    }
    let count = usize::from(u16_at(&page, 45)?);
    let max_columns = usize::from(u16_at(&page, 41)?);
    let variable_slots = usize::from(u16_at(&page, 43)?);
    if count == 0
        || count > 256
        || max_columns < count
        || max_columns > 256
        || variable_slots > max_columns
    {
        return Err(InspectError::Unsupported);
    }
    let indexes = u32_at(&page, 51)? as usize;
    if indexes > 256 {
        return Err(InspectError::StructureLimit);
    }
    let start = 63 + indexes * 12;
    let end = start + count * 25;
    if end > 4096 {
        return Err(InspectError::Unsupported);
    }
    reader
        .budget
        .charge_allocation(count as u64, std::mem::size_of::<Field>() as u64)?;
    let mut fields: Vec<Field> = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut position = end;
    let mut fixed_end = 2;
    for i in 0..count {
        reader.tick()?;
        let c = &page[start + i * 25..start + (i + 1) * 25];
        let length = usize::from(u16_at(&page, position)?);
        position += 2;
        let bytes = page
            .get(position..position + length)
            .ok_or(InspectError::Truncated)?;
        position += length;
        let name = text::decode(bytes, false, reader.budget)?;
        let field = Field {
            kind: c[0],
            flags: c[15],
            extended_flags: c[16],
            number: usize::from(u16_at(c, 5)?),
            variable: usize::from(u16_at(c, 7)?),
            fixed: c[15] & 1 != 0,
            offset: usize::from(u16_at(c, 21)?),
            length: if c[0] == 1 {
                0
            } else {
                usize::from(u16_at(c, 23)?)
            },
            name,
        };
        if field.number >= max_columns || fields.iter().any(|other| other.number == field.number) {
            return Err(InspectError::Malformed);
        }
        if field.fixed {
            fixed_end = fixed_end.max(2 + field.offset + field.length);
        } else if field.variable >= variable_slots {
            return Err(InspectError::Malformed);
        }
        fields.push(field);
    }
    let layout = Layout {
        fields,
        max_columns,
        variable_slots,
        fixed_end,
        records: u32_at(&page, 16)?,
        usage: u32_at(&page, 55)?,
    };
    Ok(layout)
}
pub(crate) fn validate_catalog(layout: &Layout) -> Result<(), InspectError> {
    for (name, kind, length) in [
        ("Id", 4, 4),
        ("ParentId", 4, 4),
        ("Type", 3, 2),
        ("Flags", 4, 4),
    ] {
        let field = layout.field(name)?;
        if field.kind != kind || !field.fixed || field.length != length {
            return Err(InspectError::Unsupported);
        }
    }
    let name = layout.field("Name")?;
    if name.kind != 10 || name.fixed {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}
