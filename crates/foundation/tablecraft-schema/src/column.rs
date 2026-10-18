use crate::ScalarType;
use tablecraft_core::values::Value;
use thiserror::Error;

/// Named scalar input column. Construction validates the name.
#[derive(Clone, PartialEq, Eq)]
pub struct Column {
    name: String,
    data_type: ScalarType,
    nullable: bool,
}
/// Schema rejection, without names or row contents.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SchemaError {
    /// Empty, oversized or control-containing name.
    #[error("invalid schema name")]
    Name,
    /// Repeated names under the initial ASCII-insensitive identity policy.
    #[error("duplicate schema column")]
    Duplicate,
    /// Empty or oversized relation schema.
    #[error("invalid schema width")]
    Width,
    /// Row does not match column count, carrier or nullability.
    #[error("row does not match schema")]
    Row,
}
impl Column {
    /// Create a column with a nonempty name of at most 255 UTF-8 bytes.
    pub fn new(name: String, data_type: ScalarType, nullable: bool) -> Result<Self, SchemaError> {
        if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
            return Err(SchemaError::Name);
        }
        Ok(Self {
            name,
            data_type,
            nullable,
        })
    }
    /// Original column spelling.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Exact input carrier.
    pub const fn data_type(&self) -> ScalarType {
        self.data_type
    }
    /// Whether NULL is permitted.
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}
/// Validate 1–256 uniquely named input columns; output aliases may be duplicated.
pub fn validate_columns(columns: &[Column]) -> Result<(), SchemaError> {
    if columns.is_empty() || columns.len() > 256 {
        return Err(SchemaError::Width);
    }
    for (i, column) in columns.iter().enumerate() {
        if columns[..i]
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&column.name))
        {
            return Err(SchemaError::Duplicate);
        }
    }
    Ok(())
}
/// Validate exact carriers and nullability without coercion.
pub fn validate_row(columns: &[Column], row: &[Value]) -> Result<(), SchemaError> {
    if columns.len() != row.len()
        || columns
            .iter()
            .zip(row)
            .any(|(c, v)| ScalarType::of(v).map_or(!c.nullable, |kind| kind != c.data_type))
    {
        return Err(SchemaError::Row);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schemas_reject_invalid_names_duplicates_and_carrier_coercion() {
        assert!(Column::new("".into(), ScalarType::Text, true).is_err());
        let c = Column::new("ID".into(), ScalarType::Int32, false).unwrap();
        assert_eq!(
            validate_row(std::slice::from_ref(&c), &[Value::Int64(1)]),
            Err(SchemaError::Row)
        );
        assert_eq!(
            validate_row(std::slice::from_ref(&c), &[Value::Null]),
            Err(SchemaError::Row)
        );
        assert!(validate_row(std::slice::from_ref(&c), &[Value::Int32(1)]).is_ok());
        assert_eq!(
            validate_columns(&[
                c,
                Column::new("id".into(), ScalarType::Int32, true).unwrap()
            ]),
            Err(SchemaError::Duplicate)
        );
        let c = Column::new("Text".into(), ScalarType::Text, true).unwrap();
        assert!(validate_row(std::slice::from_ref(&c), &[Value::Null]).is_ok());
        assert!(validate_row(&[c], &[Value::Text(String::new())]).is_ok());
    }
}
