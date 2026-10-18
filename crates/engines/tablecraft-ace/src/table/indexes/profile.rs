use super::super::schema::TableSchema;
use super::{NativeIndex, constraints::IGNORE_NULLS};
use crate::InspectError;
use tablecraft_schema::ScalarType;

pub(super) fn int32_column(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<usize, InspectError> {
    if index.columns.len() != 1 || index.flags & IGNORE_NULLS != 0 || index.constraints().is_none()
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
