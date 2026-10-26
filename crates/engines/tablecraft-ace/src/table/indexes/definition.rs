use super::super::schema::TableSchema;
use super::{IndexColumn, NativeIndex};
use crate::{
    InspectError,
    format::{
        bytes::{u16_at, u32_at},
        definition,
        layout::INDEX_KEY_COLUMNS,
        reader::Reader,
        text,
    },
};
use arrayvec::ArrayVec;
use tablecraft_schema::SchemaName;

// Jet 4 / initial ACE definition facts: pinned references in docs/access/indexes/NATIVE_INDEXES.md.
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const KEY_START: usize = 4;
const KEY_BYTES: usize = 3;
const UNUSED_COLUMN: u16 = u16::MAX;
const ASCENDING: u8 = 1;
const USAGE_OFFSET: usize = 34;
const ROOT_OFFSET: usize = 38;
const FLAGS_OFFSET: usize = 46;
const NUMBER_OFFSET: usize = 4;
const DATA_NUMBER_OFFSET: usize = 8;
const TYPE_OFFSET: usize = 23;

pub(super) fn read<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
) -> Result<Vec<NativeIndex>, InspectError> {
    let page = definition::read(reader, schema.definition)?;
    let layout = &schema.layout;
    let logical_start = layout.index_start + layout.physical_indexes * PHYSICAL_BYTES;
    let names_start = logical_start + layout.logical_indexes * LOGICAL_BYTES;
    if names_start > page.len() {
        return Err(InspectError::Unsupported);
    }
    reader.budget.charge_allocation(
        layout.logical_indexes as u64,
        size_of::<NativeIndex>() as u64,
    )?;
    let mut indexes: Vec<NativeIndex> = Vec::new();
    indexes
        .try_reserve_exact(layout.logical_indexes)
        .map_err(|_| InspectError::Allocation)?;
    let mut position = names_start;
    for i in 0..layout.logical_indexes {
        reader.tick()?;
        let logical =
            &page[logical_start + i * LOGICAL_BYTES..logical_start + (i + 1) * LOGICAL_BYTES];
        let length = usize::from(u16_at(&page, position)?);
        position += size_of::<u16>();
        let encoded = page
            .get(position..position + length)
            .ok_or(InspectError::Truncated)?;
        position += length;
        let name = SchemaName::new(text::decode(encoded, false, reader.budget)?)?;
        let number = u32_at(logical, NUMBER_OFFSET)?;
        let data_number = u32_at(logical, DATA_NUMBER_OFFSET)?;
        if indexes
            .iter()
            .any(|x| x.number == number || x.name.as_str().eq_ignore_ascii_case(name.as_str()))
            || data_number as usize >= layout.physical_indexes
        {
            return Err(InspectError::Malformed);
        }
        let start = layout.index_start + data_number as usize * PHYSICAL_BYTES;
        let physical = &page[start..start + PHYSICAL_BYTES];
        // Inline key storage is already included in the NativeIndex vector's
        // allocation charge above. Each physical definition has ten key slots.
        let mut columns = ArrayVec::<IndexColumn, INDEX_KEY_COLUMNS>::new();
        let mut unused = false;
        for key in 0..INDEX_KEY_COLUMNS {
            let at = KEY_START + key * KEY_BYTES;
            let column = u16_at(physical, at)?;
            if column == UNUSED_COLUMN {
                unused = true;
                continue;
            }
            let number = usize::from(column);
            if unused || physical[at + size_of::<u16>()] & !ASCENDING != 0 {
                return Err(InspectError::Unsupported);
            }
            if !schema.columns.iter().any(|c| c.field.number == number)
                || columns.iter().any(|c: &IndexColumn| c.number == number)
            {
                return Err(InspectError::Malformed);
            }
            columns
                .try_push(IndexColumn {
                    number,
                    descending: physical[at + size_of::<u16>()] & ASCENDING == 0,
                })
                .map_err(|_| InspectError::Unsupported)?;
        }
        if columns.is_empty() {
            return Err(InspectError::Unsupported);
        }
        let root = u32_at(physical, ROOT_OFFSET)?;
        if root == 0 || root >= reader.header.page_count() {
            return Err(InspectError::Malformed);
        }
        indexes.push(NativeIndex {
            name,
            number,
            data_number,
            root,
            usage: u32_at(physical, USAGE_OFFSET)?,
            kind: logical[TYPE_OFFSET],
            flags: physical[FLAGS_OFFSET],
            backing_primary: false,
            columns,
            reference: super::reference::read(logical, logical[TYPE_OFFSET])?,
            foreign_checked: false,
        });
    }
    // Primary semantics belong to the backing data even if another logical
    // index sharing it omits UNIQUE/REQUIRED in the physical flags.
    let mut primary_data = None;
    for index in &indexes {
        reader.tick()?;
        if index.kind == super::constraints::PRIMARY_INDEX
            && primary_data.replace(index.data_number).is_some()
        {
            return Err(InspectError::Malformed);
        }
    }
    for index in &mut indexes {
        reader.tick()?;
        index.backing_primary = primary_data == Some(index.data_number);
    }
    Ok(indexes)
}
