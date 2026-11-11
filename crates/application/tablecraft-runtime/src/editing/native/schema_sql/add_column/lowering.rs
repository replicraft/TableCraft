//! First unadorned Currency ADD lowering; no native mutation or save authority.
use crate::Error;
use tablecraft_ace::CreateColumn;
use tablecraft_schema::ScalarType;
use tablecraft_sql::query::definition::Column;

pub(super) fn capture(column: &Column) -> Result<CreateColumn<'_>, Error> {
    if column.not_null
        || column.compression
        || column.default_value.is_some()
        || column.constraint.is_some()
    {
        return Err(Error::SchemaSqlUnsupported);
    }
    let (declaration, generated) =
        super::super::declaration::capture(column.name.as_str(), &column.data_type)?;
    if generated || declaration.scalar_type() != ScalarType::Currency {
        return Err(Error::SchemaSqlUnsupported);
    }
    Ok(declaration)
}

#[cfg(test)]
mod tests;
