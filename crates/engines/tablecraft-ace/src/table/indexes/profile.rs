use super::super::schema::TableSchema;
use super::{NativeIndex, constraints::IGNORE_NULLS};
use crate::InspectError;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyWidth {
    Byte,
    Int16,
    Int32,
    Int64,
}
impl KeyWidth {
    pub const fn bytes(self) -> usize {
        match self {
            Self::Byte => size_of::<u8>(),
            Self::Int16 => size_of::<i16>(),
            Self::Int32 => size_of::<i32>(),
            Self::Int64 => size_of::<i64>(),
        }
    }
}

pub(super) fn integer_column(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<(usize, KeyWidth), InspectError> {
    if index.columns.len() != 1 || index.flags & IGNORE_NULLS != 0 || index.constraints().is_none()
    {
        return Err(InspectError::Unsupported);
    }
    let column = schema
        .columns
        .iter()
        .position(|c| c.number() == index.columns[0].number())
        .ok_or(InspectError::Malformed)?;
    let width = match schema.columns[column].scalar_type() {
        Some(ScalarType::Byte) => KeyWidth::Byte,
        Some(ScalarType::Int16) => KeyWidth::Int16,
        Some(ScalarType::Int32) => KeyWidth::Int32,
        Some(ScalarType::Int64) => KeyWidth::Int64,
        _ => return Err(InspectError::Unsupported),
    };
    Ok((column, width))
}
