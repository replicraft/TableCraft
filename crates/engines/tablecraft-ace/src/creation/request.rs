//! Borrow-only input validation, completed before image allocation.
use super::profile;
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

/// Payload-free input, resource or serialization failure for fresh creation.
#[derive(Debug, Error)]
pub enum CreateDatabaseError {
    /// A name is empty, starts with a space, contains prohibited characters or exceeds 64 UTF-16 units.
    #[error("creation requires a valid native identifier of at most 64 UTF-16 units")]
    InvalidName,
    /// Two column names are equal under the profile's ASCII folding policy.
    #[error("creation column names must be distinct under ASCII folding")]
    DuplicateColumn,
    /// The user table name conflicts with an authored system table.
    #[error("creation table name conflicts with a required system table")]
    SystemTableName,
    /// This bounded profile requires between one and sixteen user columns.
    #[error("creation requires between one and sixteen Int32 columns")]
    ColumnLimit,
    /// This bounded profile admits at most sixty-four user rows.
    #[error("creation admits at most sixty-four rows")]
    RowLimit,
    /// A row does not contain exactly the declared number of present Int32 cells.
    #[error("creation row width differs from the declared columns")]
    RowWidth,
    /// The selected rows and directory do not fit the one user DATA page.
    #[error("creation rows do not fit the single user DATA page")]
    PageCapacity,
    /// A checked internal layout bound was not satisfied; no artifact is returned.
    #[error("creation layout bounds were not satisfied")]
    Layout,
    /// Cancellation, allocation or a shared resource limit rejected the operation.
    #[error(transparent)]
    Inspect(#[from] InspectError),
}

pub(super) struct Request<'a> {
    pub table: &'a str,
    pub columns: &'a [&'a str],
    pub rows: &'a [&'a [i32]],
    pub row_bytes: usize,
    pub free_bytes: usize,
}

pub(super) fn validate<'a>(
    table: &'a str,
    columns: &'a [&'a str],
    rows: &'a [&'a [i32]],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Request<'a>, CreateDatabaseError> {
    check(budget, cancelled)?;
    name(table, budget, cancelled)?;
    for system in profile::SYSTEM_NAMES {
        compare(table, system, budget)?;
        if table.eq_ignore_ascii_case(system) {
            return Err(CreateDatabaseError::SystemTableName);
        }
    }
    if columns.is_empty() || columns.len() > profile::MAX_COLUMNS {
        return Err(CreateDatabaseError::ColumnLimit);
    }
    for (position, column) in columns.iter().enumerate() {
        name(column, budget, cancelled)?;
        for prior in &columns[..position] {
            check(budget, cancelled)?;
            compare(column, prior, budget)?;
            if column.eq_ignore_ascii_case(prior) {
                return Err(CreateDatabaseError::DuplicateColumn);
            }
        }
    }
    if rows.len() > profile::MAX_ROWS {
        return Err(CreateDatabaseError::RowLimit);
    }
    for row in rows {
        check(budget, cancelled)?;
        if row.len() != columns.len() {
            return Err(CreateDatabaseError::RowWidth);
        }
        // The borrowed i32 slice already establishes present stored carriers.
        // Account the complete validated width without cloning any row values.
        budget
            .charge(Resource::Steps, row.len() as u64)
            .map_err(InspectError::from)?;
    }

    let row_bytes = size_of::<u16>()
        .checked_add(
            columns
                .len()
                .checked_mul(size_of::<i32>())
                .ok_or(CreateDatabaseError::Layout)?,
        )
        .and_then(|length| length.checked_add(columns.len().div_ceil(u8::BITS as usize)))
        .ok_or(CreateDatabaseError::Layout)?;
    let used = rows
        .len()
        .checked_mul(row_bytes + size_of::<u16>())
        .and_then(|bytes| bytes.checked_add(super::record::DATA_HEADER_BYTES))
        .ok_or(CreateDatabaseError::PageCapacity)?;
    let free_bytes = PAGE_BYTES
        .checked_sub(used)
        .ok_or(CreateDatabaseError::PageCapacity)?;

    // All names have already been bounded; still derive and check the complete
    // variable-sized declaration before allocating the database image.
    if super::definition::length(profile::Fields::User(columns), false)? > PAGE_BYTES {
        return Err(CreateDatabaseError::PageCapacity);
    }
    check(budget, cancelled)?;

    Ok(Request {
        table,
        columns,
        rows,
        row_bytes,
        free_bytes,
    })
}

fn name(
    value: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    if value.is_empty() || value.starts_with(' ') {
        return Err(CreateDatabaseError::InvalidName);
    }
    let mut units = 0;
    for character in value.chars() {
        check(budget, cancelled)?;
        units += character.len_utf16();
        // Borrowed form of SchemaName's nonempty/control-free contract,
        // plus native identifier punctuation from the qualified creation
        // profile (Jackcess DatabaseImpl.java:271-272,2133-2151).
        if character.is_control()
            || matches!(character, '.' | '!' | '`' | '[' | ']')
            || units > profile::MAX_NAME_UNITS
        {
            return Err(CreateDatabaseError::InvalidName);
        }
    }
    Ok(())
}

fn compare(left: &str, right: &str, budget: &mut ResourceBudget) -> Result<(), InspectError> {
    budget.charge(Resource::Steps, left.len().max(right.len()) as u64)?;
    Ok(())
}
