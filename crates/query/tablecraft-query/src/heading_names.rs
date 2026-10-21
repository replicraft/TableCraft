//! Shared crosstab heading spelling; callers charge before allocating a name.
use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use std::fmt::Write;

// Decimal digits plus a sign, including i64::MIN.
pub(crate) const INTEGER_NAME_BYTES: usize = i64::MAX.ilog10() as usize + 2;

pub(crate) fn validate(text: &str) -> Result<(), QueryError> {
    if text.is_empty()
        || text.len() > SOURCE_SCHEMA_POLICY.max_name_bytes
        || text.chars().any(char::is_control)
    {
        return Err(QueryError::Unsupported);
    }
    Ok(())
}
pub(crate) fn integer(value: i64) -> Result<String, QueryError> {
    let mut name = String::new();
    name.try_reserve_exact(INTEGER_NAME_BYTES)
        .map_err(|_| QueryError::Allocation)?;
    write!(&mut name, "{value}").map_err(|_| QueryError::Allocation)?;
    Ok(name)
}
