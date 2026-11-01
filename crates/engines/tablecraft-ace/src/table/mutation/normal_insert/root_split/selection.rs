//! Metadata-only sufficient overflow bound; no mutation failure chooses a route.
use crate::{
    EditError, InspectError, NativeImage, RowInsert,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    limits::MAX_LIVE_ROWS,
    table::{
        indexes::{
            header::IndexHeader,
            page::{ENTRY_START, LEAF},
        },
        mutation::preserved_source::Source,
    },
};
use tablecraft_schema::ScalarType;

const MANUAL_FIXED_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG;
const ENTRY_BYTES: usize = size_of::<u8>() + size_of::<i32>() + size_of::<u32>();

pub(in crate::table::mutation::normal_insert) fn selected(
    image: &NativeImage,
    source: &Source,
    requests: &[RowInsert],
) -> Result<bool, EditError> {
    if requests.is_empty() || source.table.schema().layout.physical_indexes != 1 {
        return Ok(false);
    }
    let Some(index) = source.indexes.indexes().first() else {
        return Ok(false);
    };
    let Some(constraints) = index.constraints() else {
        return Ok(false);
    };
    let [key] = index.columns() else {
        return Ok(false);
    };
    if !constraints.primary_key()
        || !constraints.unique()
        || !constraints.required()
        || constraints.ignore_nulls()
        || key.descending()
    {
        return Ok(false);
    }
    let Some(field) = source.table.schema().columns().get(key.number()) else {
        return Ok(false);
    };
    if field.number() != key.number()
        || field.scalar_type() != Some(ScalarType::Int32)
        || field.flags() != MANUAL_FIXED_FLAGS
    {
        return Ok(false);
    }
    let at = index.root_page() as usize * PAGE_BYTES;
    let Some(page) = image.as_bytes().get(at..at + PAGE_BYTES) else {
        return Err(EditError::Preservation);
    };
    if page[0] != LEAF {
        return Ok(false);
    }
    let count = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    if count > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let seed = usize::from(IndexHeader::read(page)?.prefix.get()).min(ENTRY_BYTES - 1);
    let minimum = (count - 1)
        .checked_mul(ENTRY_BYTES - seed)
        .and_then(|length| length.checked_add(ENTRY_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    Ok(minimum > PAGE_BYTES - ENTRY_START)
}
