use super::{buffer, source::Original};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        bytes::{u16_at, u32_at},
        definition::{LENGTH_OFFSET, NEXT_PAGE_OFFSET, PAGE_HEADER_BYTES},
        layout::{
            COLUMN_BLOCK_OFFSET, INDEX_COUNT_OFFSET, INDEX_ENTRY_BYTES, INDEX_KEY_COLUMNS,
            LOGICAL_INDEX_COUNT_OFFSET,
        },
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::indexes::{
        definition::{
            ASCENDING, DATA_NUMBER_OFFSET, FLAGS_OFFSET, KEY_BYTES, KEY_START, LOGICAL_BYTES,
            NUMBER_OFFSET, PHYSICAL_BYTES, ROOT_OFFSET, TYPE_OFFSET, UNUSED_COLUMN, USAGE_OFFSET,
        },
        definitions_with_tail,
    },
};
use std::ops::Range;
use tablecraft_core::limits::ResourceBudget;

const FREE_OFFSET: usize = 2;
const INDEX_MAGIC: u32 = 1923;
const LOGICAL_MAGIC: u32 = 1625;
const ORDINARY_FLAGS: u8 = 0x80;
const RELATED_INDEX_OFFSET: usize = 13;
const NEW_COUNTER_OFFSET: usize = 4;
const TRAILER: [u8; 2] = [0xff, 0xff];

pub(in crate::table) struct DefinitionLayout {
    pub definition: u32,
    pub length: usize,
    pub columns: Range<usize>,
    pub physical: Range<usize>,
    pub logical: Range<usize>,
    pub names: Range<usize>,
    pub physical_count: u32,
    pub logical_count: u32,
}

pub(super) struct CreatedDescriptor {
    pub physical: u32,
    pub logical: u32,
    pub usage: u32,
    pub root: u32,
}

impl DefinitionLayout {
    pub(super) fn read(
        image: &NativeImage,
        checked: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let schema = checked.schema();
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let raw = reader.page(schema.definition_page())?;
        let length = usize::try_from(u32_at(&raw, LENGTH_OFFSET)?)
            .map_err(|_| InspectError::StructureLimit)?;
        if u32_at(&raw, NEXT_PAGE_OFFSET)? != 0
            || !(PAGE_HEADER_BYTES..=PAGE_BYTES).contains(&length)
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let definitions = definitions_with_tail(&mut reader, schema)?;
        let tail = definitions.tail;
        if tail.checked_add(TRAILER.len()) != Some(length)
            || raw.get(tail..length) != Some(TRAILER.as_slice())
            || usize::from(u16_at(&raw, FREE_OFFSET)?)
                != PAGE_BYTES.saturating_sub(length + PAGE_HEADER_BYTES)
            || definitions.indexes.len() != schema.layout.logical_indexes
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let physical_count = schema.layout.physical_indexes;
        let logical_count = schema.layout.logical_indexes;
        for physical in 0..physical_count {
            reader.tick()?;
            reader.work(definitions.indexes.len())?;
            if !definitions
                .indexes
                .iter()
                .any(|index| index.data_number() as usize == physical)
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
        }
        let column_start = COLUMN_BLOCK_OFFSET
            .checked_add(
                physical_count
                    .checked_mul(INDEX_ENTRY_BYTES)
                    .ok_or(InspectError::StructureLimit)?,
            )
            .ok_or(InspectError::StructureLimit)?;
        let physical_end = schema
            .layout
            .index_start
            .checked_add(
                physical_count
                    .checked_mul(PHYSICAL_BYTES)
                    .ok_or(InspectError::StructureLimit)?,
            )
            .ok_or(InspectError::StructureLimit)?;
        let logical_end = physical_end
            .checked_add(
                logical_count
                    .checked_mul(LOGICAL_BYTES)
                    .ok_or(InspectError::StructureLimit)?,
            )
            .ok_or(InspectError::StructureLimit)?;
        if !(column_start <= schema.layout.index_start
            && schema.layout.index_start <= physical_end
            && physical_end <= logical_end
            && logical_end <= tail)
        {
            return Err(InspectError::Malformed.into());
        }
        Ok(Self {
            definition: schema.definition_page(),
            length,
            columns: column_start..schema.layout.index_start,
            physical: schema.layout.index_start..physical_end,
            logical: physical_end..logical_end,
            names: logical_end..tail,
            physical_count: u32::try_from(physical_count)
                .map_err(|_| InspectError::StructureLimit)?,
            logical_count: u32::try_from(logical_count)
                .map_err(|_| InspectError::StructureLimit)?,
        })
    }

    pub(super) fn new_length(&self, name_bytes: usize) -> Result<usize, EditError> {
        let length = self
            .length
            .checked_add(INDEX_ENTRY_BYTES)
            .and_then(|length| length.checked_add(PHYSICAL_BYTES))
            .and_then(|length| length.checked_add(LOGICAL_BYTES))
            .and_then(|length| length.checked_add(size_of::<u16>()))
            .and_then(|length| length.checked_add(name_bytes))
            .ok_or(InspectError::StructureLimit)?;
        if length > PAGE_BYTES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        Ok(length)
    }
}

pub(super) fn compose(
    image: &NativeImage,
    original: &Original,
    descriptor: CreatedDescriptor,
    counter: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let layout = &original.layout;
    let name_units = original.name.as_str().encode_utf16().count();
    let name_bytes = name_units
        .checked_mul(size_of::<u16>())
        .ok_or(InspectError::StructureLimit)?;
    let length = layout.new_length(name_bytes)?;
    if descriptor.physical != layout.physical_count || descriptor.logical != layout.logical_count {
        return Err(EditError::Preservation);
    }
    let start = layout.definition as usize * PAGE_BYTES;
    let raw = image
        .as_bytes()
        .get(start..start + PAGE_BYTES)
        .ok_or(InspectError::Truncated)?;
    let mut active = buffer::vector::<u8>(length, budget, cancelled)?;
    active.extend_from_slice(&raw[..layout.columns.start]);
    let mut count = [0; INDEX_ENTRY_BYTES];
    count[NEW_COUNTER_OFFSET..NEW_COUNTER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&counter.to_le_bytes());
    active.extend_from_slice(&count);
    active.extend_from_slice(&raw[layout.columns.clone()]);
    active.extend_from_slice(&raw[layout.physical.clone()]);
    let mut physical = [0; PHYSICAL_BYTES];
    physical[..size_of::<u32>()].copy_from_slice(&INDEX_MAGIC.to_le_bytes());
    for key in 0..INDEX_KEY_COLUMNS {
        check(budget, cancelled)?;
        let at = KEY_START + key * KEY_BYTES;
        physical[at..at + size_of::<u16>()].copy_from_slice(&UNUSED_COLUMN.to_le_bytes());
    }
    let column = u16::try_from(original.column).map_err(|_| InspectError::StructureLimit)?;
    physical[KEY_START..KEY_START + size_of::<u16>()].copy_from_slice(&column.to_le_bytes());
    physical[KEY_START + size_of::<u16>()] = if original.descending { 0 } else { ASCENDING };
    physical[USAGE_OFFSET..USAGE_OFFSET + size_of::<u32>()]
        .copy_from_slice(&descriptor.usage.to_le_bytes());
    physical[ROOT_OFFSET..ROOT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&descriptor.root.to_le_bytes());
    physical[FLAGS_OFFSET] = ORDINARY_FLAGS;
    active.extend_from_slice(&physical);
    active.extend_from_slice(&raw[layout.logical.clone()]);
    let mut logical = [0; LOGICAL_BYTES];
    logical[..size_of::<u32>()].copy_from_slice(&LOGICAL_MAGIC.to_le_bytes());
    logical[NUMBER_OFFSET..NUMBER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&descriptor.logical.to_le_bytes());
    logical[DATA_NUMBER_OFFSET..DATA_NUMBER_OFFSET + size_of::<u32>()]
        .copy_from_slice(&descriptor.physical.to_le_bytes());
    logical[RELATED_INDEX_OFFSET..RELATED_INDEX_OFFSET + size_of::<u32>()]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    logical[TYPE_OFFSET] = 0;
    active.extend_from_slice(&logical);
    active.extend_from_slice(&raw[layout.names.clone()]);
    active.extend_from_slice(
        &u16::try_from(name_bytes)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    for unit in original.name.as_str().encode_utf16() {
        check(budget, cancelled)?;
        active.extend_from_slice(&unit.to_le_bytes());
    }
    active.extend_from_slice(&TRAILER);
    if active.len() != length {
        return Err(EditError::Preservation);
    }
    active[LENGTH_OFFSET..LENGTH_OFFSET + size_of::<u32>()]
        .copy_from_slice(&(length as u32).to_le_bytes());
    active[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()].copy_from_slice(
        &u16::try_from(PAGE_BYTES.saturating_sub(length + PAGE_HEADER_BYTES))
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    active[INDEX_COUNT_OFFSET..INDEX_COUNT_OFFSET + size_of::<u32>()].copy_from_slice(
        &layout
            .physical_count
            .checked_add(1)
            .ok_or(InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    active[LOGICAL_INDEX_COUNT_OFFSET..LOGICAL_INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(
            &layout
                .logical_count
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    let mut result = buffer::copy(raw, budget, cancelled)?;
    result[..length].copy_from_slice(&active);
    Ok(result)
}
