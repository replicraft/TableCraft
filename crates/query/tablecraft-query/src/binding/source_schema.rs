//! Validate explicit supported/unavailable source field declarations together.
use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_schema::{Column, SchemaError, SchemaName};

pub(super) fn validate(columns: &[Column], unavailable: &[SchemaName]) -> Result<(), QueryError> {
    if !columns.is_empty() || unavailable.is_empty() {
        SOURCE_SCHEMA_POLICY.validate_columns(columns)?;
    }
    if columns
        .len()
        .checked_add(unavailable.len())
        .is_none_or(|width| width > SOURCE_SCHEMA_POLICY.max_columns)
    {
        return Err(SchemaError::Width.into());
    }
    for (index, name) in unavailable.iter().enumerate() {
        SOURCE_SCHEMA_POLICY.validate_name(name)?;
        if columns.iter().any(|column| {
            SOURCE_SCHEMA_POLICY
                .names
                .equals(column.name(), name.as_str())
        }) || unavailable[..index].iter().any(|other| {
            SOURCE_SCHEMA_POLICY
                .names
                .equals(other.as_str(), name.as_str())
        }) {
            return Err(SchemaError::Duplicate.into());
        }
    }
    Ok(())
}
