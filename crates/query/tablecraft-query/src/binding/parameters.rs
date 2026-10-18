use super::Parameter;
use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_schema::{ScalarType, SchemaName, TypeDescriptor};

pub(super) fn bind(
    declarations: &[tablecraft_sql::query::Parameter],
) -> Result<Vec<Parameter>, QueryError> {
    let mut parameters: Vec<Parameter> = Vec::new();
    for parameter in declarations {
        let name = SchemaName::new(parameter.name.as_str().into())?;
        SOURCE_SCHEMA_POLICY
            .validate_name(&name)
            .map_err(|_| QueryError::StructureLimit)?;
        if parameters.iter().any(|p| {
            SOURCE_SCHEMA_POLICY
                .names
                .equals(p.name.as_str(), name.as_str())
        }) {
            return Err(QueryError::AmbiguousName);
        }
        let kind = match parameter.data_type.name.as_str() {
            "BIT" | "YESNO" | "BOOLEAN" | "LOGICAL" => ScalarType::Boolean,
            "BYTE" | "TINYINT" => ScalarType::Byte,
            "SHORT" | "SMALLINT" => ScalarType::Int16,
            "LONG" | "LONGINTEGER" | "INTEGER" | "INT" => ScalarType::Int32,
            "BIGINT" | "LONGLONG" | "LARGE NUMBER" => ScalarType::Int64,
            "SINGLE" | "REAL" | "IEEESINGLE" => ScalarType::Single,
            "DOUBLE" | "FLOAT" | "IEEEDOUBLE" => ScalarType::Double,
            "CURRENCY" | "MONEY" => ScalarType::Currency,
            "DECIMAL" | "NUMERIC" => ScalarType::Decimal,
            "TEXT" | "CHAR" | "CHARACTER" | "VARCHAR" | "LONGTEXT" | "MEMO" | "LONGCHAR" => {
                ScalarType::Text
            }
            "GUID" | "UNIQUEIDENTIFIER" => ScalarType::Guid,
            "DATETIME" | "DATE" | "TIME" | "TIMESTAMP" => ScalarType::DateTime,
            "DATETIMEEXTENDED" => ScalarType::DateTimeExtended,
            "BINARY" | "IMAGE" | "LONGBINARY" => ScalarType::Binary,
            _ => return Err(QueryError::Unsupported),
        };
        let data_type = match parameter.data_type.arguments.as_slice() {
            [] => TypeDescriptor::new(kind),
            [n] if kind == ScalarType::Text && *n > 0 => TypeDescriptor::text(*n as usize)?,
            _ => return Err(QueryError::Unsupported),
        };
        parameters.push(Parameter { name, data_type });
    }
    Ok(parameters)
}
