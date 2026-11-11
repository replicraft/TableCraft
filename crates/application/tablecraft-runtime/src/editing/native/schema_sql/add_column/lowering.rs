//! Unadorned scalar ADD lowering; no native mutation or save authority.
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
    if generated
        || !matches!(
            declaration.scalar_type(),
            ScalarType::Boolean
                | ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Single
                | ScalarType::Double
                | ScalarType::Currency
                | ScalarType::DateTime
                | ScalarType::Guid
                | ScalarType::Text
                | ScalarType::Decimal
        )
    {
        return Err(Error::SchemaSqlUnsupported);
    }
    Ok(declaration)
}

#[cfg(test)]
mod tests;
