use crate::{Column, NameComparison, SchemaError, SchemaName, validate_columns};

/// Explicit limits and name comparison for a consumer/backend profile.
/// There is no implicit query-sized default or native Access qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaPolicy {
    /// Maximum UTF-8 bytes in a schema identity.
    pub max_name_bytes: usize,
    /// Maximum input relation width.
    pub max_columns: usize,
    /// Name comparison for duplicate checks and host lookup.
    pub names: NameComparison,
}
impl SchemaPolicy {
    /// Apply the profile's byte ceiling to an already validated identity.
    pub fn validate_name(&self, name: &SchemaName) -> Result<(), SchemaError> {
        if name.as_str().len() > self.max_name_bytes {
            return Err(SchemaError::Name);
        }
        Ok(())
    }
    /// Apply width/name limits and explicit duplicate-name comparison.
    pub fn validate_columns(&self, columns: &[Column]) -> Result<(), SchemaError> {
        if columns.len() > self.max_columns {
            return Err(SchemaError::Width);
        }
        for column in columns {
            self.validate_name(column.schema_name())?;
        }
        validate_columns(columns, self.names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScalarType;
    #[test]
    fn profiles_choose_limits_and_comparison_independently() {
        let columns = vec![
            Column::new("ID".into(), ScalarType::Int64, false).unwrap(),
            Column::new("id".into(), ScalarType::Int64, false).unwrap(),
        ];
        let mut policy = SchemaPolicy {
            max_name_bytes: 2,
            max_columns: 2,
            names: NameComparison::Exact,
        };
        assert!(policy.validate_columns(&columns).is_ok());
        policy.names = NameComparison::AsciiInsensitive;
        assert_eq!(
            policy.validate_columns(&columns),
            Err(SchemaError::Duplicate)
        );
        policy.names = NameComparison::Exact;
        policy.max_columns = 1;
        assert_eq!(policy.validate_columns(&columns), Err(SchemaError::Width));
        policy.max_columns = 2;
        policy.max_name_bytes = 1;
        assert_eq!(policy.validate_columns(&columns), Err(SchemaError::Name));
        let wider = (0..257)
            .map(|i| Column::new(format!("C{i}"), ScalarType::Int64, false).unwrap())
            .collect::<Vec<_>>();
        assert!(validate_columns(&wider, NameComparison::Exact).is_ok());
    }
}
