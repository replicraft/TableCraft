use super::{
    bytes::{u16_at, u32_at},
    reader::Reader,
    text,
};
use super::{data_type, definition, row::COLUMN_COUNT_BYTES};
use crate::InspectError;
use crate::limits::{MAX_COLUMNS, MAX_INDEXES};

// Jet 4 / initial ACE table/column definition offsets; see docs/access/storage/NATIVE_TABLES.md.
const RECORD_COUNT_OFFSET: usize = 16;
const MAX_COLUMN_COUNT_OFFSET: usize = 41;
const VARIABLE_SLOT_COUNT_OFFSET: usize = 43;
const COLUMN_COUNT_OFFSET: usize = 45;
const INDEX_COUNT_OFFSET: usize = 51;
const LOGICAL_INDEX_COUNT_OFFSET: usize = 47;
const OWNED_MAP_OFFSET: usize = 55;
const COLUMN_BLOCK_OFFSET: usize = 63;
const INDEX_ENTRY_BYTES: usize = 12;
const COLUMN_ENTRY_BYTES: usize = 25;
const COLUMN_TYPE_OFFSET: usize = 0;
const COLUMN_NUMBER_OFFSET: usize = 5;
const VARIABLE_INDEX_OFFSET: usize = 7;
const NUMERIC_PRECISION_OFFSET: usize = 11;
const NUMERIC_SCALE_OFFSET: usize = 12;
const COMPLEX_TYPE_ID_OFFSET: usize = 11;
const COLUMN_SORT_ORDER_OFFSET: usize = 11;
const COLUMN_FLAGS_OFFSET: usize = 15;
const COLUMN_EXTENDED_FLAGS_OFFSET: usize = 16;
const FIXED_DATA_OFFSET: usize = 21;
const COLUMN_LENGTH_OFFSET: usize = 23;
pub(crate) const FIXED_FLAG: u8 = 0x01;
pub(crate) const UPDATABLE_FLAG: u8 = 0x02;
pub(crate) const UNICODE_COMPRESSION_FLAG: u8 = 0x01;
pub(crate) const COMPLEX_FK_EXT_FLAG: u8 = 0x08;
pub(crate) const CALCULATED_EXT_MASK: u8 = 0xc0;
pub(crate) const AUTO_NUMBER_FLAG: u8 = 0x04;
pub(crate) const AUTO_NUMBER_GUID_FLAG: u8 = 0x40;
pub(crate) const HYPERLINK_FLAG: u8 = 0x80;
/// Maximum physical index key bytes, before row/child references.
pub(crate) const INDEX_KEY_BYTES: usize = 510;

pub(crate) const INDEX_KEY_COLUMNS: usize = 10;

#[derive(PartialEq, Eq)]
pub(crate) struct Field {
    pub number: usize,
    pub variable: usize,
    pub fixed: bool,
    pub offset: usize,
    pub length: usize,
    pub kind: u8,
    pub precision: u8,
    pub scale: u8,
    pub name: String,
    pub flags: u8,
    pub extended_flags: u8,
    pub complex_type_id: Option<u32>,
    pub sort_order: Option<crate::NativeSortOrder>,
}
#[derive(PartialEq, Eq)]
pub(crate) struct Layout {
    pub fields: Vec<Field>,
    pub max_columns: usize,
    pub variable_slots: usize,
    pub fixed_end: usize,
    pub records: u32,
    pub usage: u32,
    pub index_start: usize,
    pub physical_indexes: usize,
    pub logical_indexes: usize,
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
pub(crate) fn read<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    definition: u32,
) -> Result<Layout, InspectError> {
    let page = definition::read(reader, definition)?;
    let count = usize::from(u16_at(&page, COLUMN_COUNT_OFFSET)?);
    let max_columns = usize::from(u16_at(&page, MAX_COLUMN_COUNT_OFFSET)?);
    let variable_slots = usize::from(u16_at(&page, VARIABLE_SLOT_COUNT_OFFSET)?);
    if count == 0
        || count > MAX_COLUMNS
        || max_columns < count
        || max_columns > MAX_COLUMNS
        || variable_slots > max_columns
    {
        return Err(InspectError::Unsupported);
    }
    let indexes = u32_at(&page, INDEX_COUNT_OFFSET)? as usize;
    let logical_indexes = u32_at(&page, LOGICAL_INDEX_COUNT_OFFSET)? as usize;
    if indexes > MAX_INDEXES || logical_indexes > MAX_INDEXES {
        return Err(InspectError::StructureLimit);
    }
    let start = COLUMN_BLOCK_OFFSET + indexes * INDEX_ENTRY_BYTES;
    let end = start + count * COLUMN_ENTRY_BYTES;
    if end > page.len() {
        return Err(InspectError::Truncated);
    }
    reader
        .budget
        .charge_allocation(count as u64, std::mem::size_of::<Field>() as u64)?;
    let mut fields: Vec<Field> = Vec::new();
    fields
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut position = end;
    let mut fixed_end = COLUMN_COUNT_BYTES;
    for i in 0..count {
        reader.tick()?;
        let c = &page[start + i * COLUMN_ENTRY_BYTES..start + (i + 1) * COLUMN_ENTRY_BYTES];
        let length = usize::from(u16_at(&page, position)?);
        position += size_of::<u16>();
        let bytes = page
            .get(position..position + length)
            .ok_or(InspectError::Truncated)?;
        position += length;
        let name = text::decode(bytes, false, reader.budget)?;
        let field = Field {
            sort_order: if matches!(
                c[COLUMN_TYPE_OFFSET],
                data_type::SHORT_TEXT | data_type::LONG_TEXT
            ) {
                Some(crate::NativeSortOrder::read(
                    &c[COLUMN_SORT_ORDER_OFFSET..],
                )?)
            } else {
                None
            },
            kind: c[COLUMN_TYPE_OFFSET],
            precision: c[NUMERIC_PRECISION_OFFSET],
            scale: c[NUMERIC_SCALE_OFFSET],
            flags: c[COLUMN_FLAGS_OFFSET],
            extended_flags: c[COLUMN_EXTENDED_FLAGS_OFFSET],
            complex_type_id: if c[COLUMN_TYPE_OFFSET] == data_type::COMPLEX {
                Some(u32_at(c, COMPLEX_TYPE_ID_OFFSET)?)
            } else {
                None
            },
            number: usize::from(u16_at(c, COLUMN_NUMBER_OFFSET)?),
            variable: usize::from(u16_at(c, VARIABLE_INDEX_OFFSET)?),
            fixed: c[COLUMN_FLAGS_OFFSET] & FIXED_FLAG != 0,
            offset: usize::from(u16_at(c, FIXED_DATA_OFFSET)?),
            length: if c[COLUMN_TYPE_OFFSET] == data_type::BOOLEAN
                && c[COLUMN_FLAGS_OFFSET] & FIXED_FLAG != 0
            {
                0
            } else {
                usize::from(u16_at(c, COLUMN_LENGTH_OFFSET)?)
            },
            name,
        };
        if field.number >= max_columns || fields.iter().any(|other| other.number == field.number) {
            return Err(InspectError::Malformed);
        }
        if field.fixed {
            fixed_end = fixed_end.max(COLUMN_COUNT_BYTES + field.offset + field.length);
        } else {
            if field.variable >= variable_slots
                || fields
                    .iter()
                    .any(|other| !other.fixed && other.variable == field.variable)
            {
                return Err(InspectError::Malformed);
            }
        }
        fields.push(field);
    }
    let layout = Layout {
        fields,
        max_columns,
        variable_slots,
        fixed_end,
        records: u32_at(&page, RECORD_COUNT_OFFSET)?,
        usage: u32_at(&page, OWNED_MAP_OFFSET)?,
        index_start: position,
        physical_indexes: indexes,
        logical_indexes,
    };
    Ok(layout)
}
pub(crate) fn validate_catalog(layout: &Layout) -> Result<(), InspectError> {
    for (name, kind, length) in [
        ("Id", data_type::INT32, data_type::INT32_BYTES),
        ("ParentId", data_type::INT32, data_type::INT32_BYTES),
        ("Type", data_type::INT16, data_type::INT16_BYTES),
        ("Flags", data_type::INT32, data_type::INT32_BYTES),
    ] {
        let field = layout.field(name)?;
        if field.kind != kind || !field.fixed || field.length != length {
            return Err(InspectError::Unsupported);
        }
    }
    let name = layout.field("Name")?;
    if name.kind != data_type::SHORT_TEXT || name.fixed {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}
