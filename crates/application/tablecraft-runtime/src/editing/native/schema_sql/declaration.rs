//! Exact SQL aliases and native declaration sizes, without field policy defaults.
use crate::{Error, editing::EditError};
use tablecraft_ace::CreateColumn;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_sql::query::DataType;

pub(super) fn capture<'a>(
    name: &'a str,
    kind: &DataType,
) -> Result<(CreateColumn<'a>, bool), Error> {
    let (column, generated) = match (kind.name.as_str(), kind.arguments.as_slice()) {
        ("COUNTER" | "AUTOINCREMENT", []) => (CreateColumn::new(name, ScalarType::Int32), true),
        ("BIT" | "BOOLEAN" | "YESNO" | "LOGICAL" | "LOGICAL1", []) => {
            (CreateColumn::new(name, ScalarType::Boolean), false)
        }
        ("BYTE" | "TINYINT" | "INTEGER1", []) => (CreateColumn::new(name, ScalarType::Byte), false),
        ("SHORT" | "SMALLINT" | "INTEGER2", []) => {
            (CreateColumn::new(name, ScalarType::Int16), false)
        }
        ("LONG" | "INTEGER" | "INT" | "LONGINTEGER" | "INTEGER4", []) => {
            (CreateColumn::new(name, ScalarType::Int32), false)
        }
        ("BIGINT" | "LONGLONG", []) => (CreateColumn::new(name, ScalarType::Int64), false),
        ("SINGLE" | "IEEESINGLE" | "REAL" | "FLOAT4", []) => {
            (CreateColumn::new(name, ScalarType::Single), false)
        }
        ("FLOAT" | "DOUBLE" | "IEEEDOUBLE" | "FLOAT8" | "NUMBER", []) => {
            (CreateColumn::new(name, ScalarType::Double), false)
        }
        ("CURRENCY" | "MONEY", []) => (CreateColumn::new(name, ScalarType::Currency), false),
        ("DATETIME" | "DATE" | "TIME", []) => {
            (CreateColumn::new(name, ScalarType::DateTime), false)
        }
        ("GUID" | "UNIQUEIDENTIFIER", []) => (CreateColumn::new(name, ScalarType::Guid), false),
        (
            "CHAR" | "CHARACTER" | "TEXT" | "VARCHAR" | "ALPHANUMERIC" | "STRING" | "NCHAR",
            [units],
        ) => (CreateColumn::short_text(name, *units as usize)?, false),
        ("BINARY" | "VARBINARY", [bytes]) => {
            (CreateColumn::short_binary(name, *bytes as usize)?, false)
        }
        ("DECIMAL" | "NUMERIC" | "DEC", [precision, scale]) => {
            let precision = u8::try_from(*precision).map_err(|_| EditError::InvalidInput)?;
            let scale = u8::try_from(*scale).map_err(|_| EditError::InvalidInput)?;
            let constraints =
                DecimalConstraints::new(precision, scale).map_err(|_| EditError::InvalidInput)?;
            (CreateColumn::decimal(name, constraints), false)
        }
        // Unsized TEXT denotes Long Text in ACE SQL, not a guessed Short Text255.
        // Other omitted parameters require their own established native defaults.
        _ => return Err(Error::SchemaSqlUnsupported),
    };
    Ok((column, generated))
}
