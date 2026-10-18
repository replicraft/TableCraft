use super::super::schema::TableSchema;
use super::NativeIndex;
use crate::InspectError;
use tablecraft_schema::ScalarType;

const IGNORE_NULLS: u8 = 0x02;
const KNOWN_FLAGS: u8 = 0x80 | 0x01 | IGNORE_NULLS | 0x08;

pub(super) fn int32_column(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<usize, InspectError> {
    if index.columns.len() != 1
        || index.kind > 1
        || index.flags & IGNORE_NULLS != 0
        || index.flags & !KNOWN_FLAGS != 0
    {
        return Err(InspectError::Unsupported);
    }
    let column = schema
        .columns
        .iter()
        .position(|c| c.number() == index.columns[0].number())
        .ok_or(InspectError::Malformed)?;
    if schema.columns[column].scalar_type() != Some(ScalarType::Int32) {
        return Err(InspectError::Unsupported);
    }
    Ok(column)
}
