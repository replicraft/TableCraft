use super::{EditError, WriteBlocker};
use crate::{
    TableSchema,
    format::layout::{FIXED_FLAG, UPDATABLE_FLAG},
};
use tablecraft_schema::ScalarType;

pub(super) fn fixed_column(
    schema: &TableSchema,
    name: &str,
    kind: ScalarType,
    width: usize,
) -> Result<usize, EditError> {
    let position = schema
        .columns
        .iter()
        .position(|c| c.name().eq_ignore_ascii_case(name))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    fixed_column_at(schema, position, kind, width)
}

pub(super) fn fixed_column_at(
    schema: &TableSchema,
    position: usize,
    kind: ScalarType,
    width: usize,
) -> Result<usize, EditError> {
    let selected = schema
        .columns
        .get(position)
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if selected.scalar_type() != Some(kind)
        || selected.field.length != width
        || selected.field.flags != (FIXED_FLAG | UPDATABLE_FLAG)
        || selected.field.extended_flags != 0
        || !selected.fixed()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    // Boolean values have no payload region. Their native offset is unused.
    if width != 0 {
        let end = selected
            .field
            .offset
            .checked_add(width)
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if schema.columns.iter().enumerate().any(|(other, c)| {
            other != position
                && c.fixed()
                && c.field.length != 0
                && c.field.offset < end
                && c.field.offset + c.field.length > selected.field.offset
        }) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    Ok(position)
}
