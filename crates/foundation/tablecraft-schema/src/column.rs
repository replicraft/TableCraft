use crate::{NameComparison, ScalarType, SchemaError, SchemaName, TypeDescriptor};
use tablecraft_core::values::Value;

/// Named scalar input column with validated identity and declared type.
#[derive(Clone, PartialEq, Eq)]
pub struct Column {
    name: SchemaName,
    data_type: TypeDescriptor,
    nullable: bool,
}
impl Column {
    /// Create a column with an unconstrained exact carrier. Backend/consumer
    /// size and comparison limits are applied separately through SchemaPolicy.
    pub fn new(name: String, data_type: ScalarType, nullable: bool) -> Result<Self, SchemaError> {
        Self::with_type(name, TypeDescriptor::new(data_type), nullable)
    }
    /// Create a column using reusable declared type constraints.
    pub fn with_type(
        name: String,
        data_type: TypeDescriptor,
        nullable: bool,
    ) -> Result<Self, SchemaError> {
        Ok(Self {
            name: SchemaName::new(name)?,
            data_type,
            nullable,
        })
    }
    /// Original column spelling.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    /// Shared validated identity, before consumer profile limits.
    pub fn schema_name(&self) -> &SchemaName {
        &self.name
    }
    /// Exact input carrier.
    pub const fn data_type(&self) -> ScalarType {
        self.data_type.scalar_type()
    }
    /// Declared type including supported value constraints.
    pub const fn type_descriptor(&self) -> TypeDescriptor {
        self.data_type
    }
    /// Whether NULL is permitted.
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}
/// Validate a nonempty input schema using an explicit name comparison policy.
/// Width/name size limits are separate SchemaPolicy responsibilities.
/// Callers must bound input size before this quadratic duplicate check.
pub fn validate_columns(columns: &[Column], names: NameComparison) -> Result<(), SchemaError> {
    validate_columns_with(columns, names, || Ok::<(), SchemaError>(()))
}

/// Validate the same schema rules with caller control inside duplicate-name traversal.
/// The callback runs before validation, each column and every name comparison;
/// it can charge cumulative work and stop promptly on cancellation.
///
/// # Errors
/// Returns an empty/duplicate schema error or the exact caller control error.
pub fn validate_columns_with<E: From<SchemaError>>(
    columns: &[Column],
    names: NameComparison,
    mut check: impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    check()?;
    if columns.is_empty() {
        return Err(SchemaError::Width.into());
    }
    for (index, column) in columns.iter().enumerate() {
        check()?;
        for previous in &columns[..index] {
            check()?;
            if names.equals(previous.name(), column.name()) {
                return Err(SchemaError::Duplicate.into());
            }
        }
    }
    Ok(())
}
/// Validate declared carriers/constraints and nullability without coercion.
pub fn validate_row(columns: &[Column], row: &[Value]) -> Result<(), SchemaError> {
    if columns.len() != row.len()
        || columns.iter().zip(row).any(|(column, value)| {
            (!column.nullable && matches!(value, Value::Null))
                || column.data_type.validate_value(value).is_err()
        })
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
            validate_columns(
                &[
                    c,
                    Column::new("id".into(), ScalarType::Int32, true).unwrap()
                ],
                NameComparison::AsciiInsensitive
            ),
            Err(SchemaError::Duplicate)
        );
    }
    #[test]
    fn field_constraints_share_type_validation_and_keep_nullability_local() {
        let c = Column::with_type("Label".into(), TypeDescriptor::text(2).unwrap(), false).unwrap();
        assert!(validate_row(std::slice::from_ref(&c), &[Value::Text("é🦀".into())]).is_ok());
        assert_eq!(
            validate_row(std::slice::from_ref(&c), &[Value::Text("abc".into())]),
            Err(SchemaError::Row)
        );
        assert_eq!(
            validate_row(std::slice::from_ref(&c), &[Value::Null]),
            Err(SchemaError::Row)
        );
        let c = Column::with_type("Label".into(), TypeDescriptor::text(2).unwrap(), true).unwrap();
        assert!(validate_row(&[c], &[Value::Null]).is_ok());
    }

    #[test]
    fn controlled_schema_validation_stops_inside_duplicate_traversal() {
        let columns =
            ["A", "B", "C"].map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap());
        let mut calls = 0;
        let result = validate_columns_with(&columns, NameComparison::Exact, || {
            calls += 1;
            if calls == 5 {
                Err(SchemaError::Value)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(SchemaError::Value));
        assert_eq!(calls, 5);
        assert!(validate_columns(&columns, NameComparison::Exact).is_ok());
    }
}
