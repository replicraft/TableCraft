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

pub(super) fn validate_multivalues(
    columns: &[Column],
    unavailable: &[SchemaName],
    values: &[Column],
) -> Result<(), QueryError> {
    if values.is_empty() {
        return Ok(());
    }
    SOURCE_SCHEMA_POLICY.validate_columns(values)?;
    if columns
        .len()
        .checked_add(unavailable.len())
        .and_then(|n| n.checked_add(values.len()))
        .is_none_or(|n| n > SOURCE_SCHEMA_POLICY.max_columns)
    {
        return Err(SchemaError::Width.into());
    }
    for column in values {
        if !unavailable.iter().any(|name| {
            SOURCE_SCHEMA_POLICY
                .names
                .equals(name.as_str(), column.name())
        }) {
            return Err(QueryError::Source);
        }
    }
    Ok(())
}

pub(super) fn validate_members(
    columns: &[Column],
    unavailable: &[SchemaName],
    values: &[Column],
    members: &[tablecraft_schema::CollectionMember],
) -> Result<(), QueryError> {
    if columns
        .len()
        .checked_add(unavailable.len())
        .and_then(|n| n.checked_add(values.len()))
        .and_then(|n| n.checked_add(members.len()))
        .is_none_or(|n| n > SOURCE_SCHEMA_POLICY.max_columns)
    {
        return Err(SchemaError::Width.into());
    }
    for (index, member) in members.iter().enumerate() {
        SOURCE_SCHEMA_POLICY.validate_name(member.collection())?;
        SOURCE_SCHEMA_POLICY.validate_name(member.column().schema_name())?;
        let collection = member.collection().as_str();
        if !unavailable
            .iter()
            .any(|name| SOURCE_SCHEMA_POLICY.names.equals(name.as_str(), collection))
            || values
                .iter()
                .any(|value| SOURCE_SCHEMA_POLICY.names.equals(value.name(), collection))
        {
            return Err(QueryError::Source);
        }
        if members[..index].iter().any(|other| {
            SOURCE_SCHEMA_POLICY
                .names
                .equals(collection, other.collection().as_str())
                && SOURCE_SCHEMA_POLICY
                    .names
                    .equals(member.column().name(), other.column().name())
        }) {
            return Err(SchemaError::Duplicate.into());
        }
    }
    Ok(())
}
