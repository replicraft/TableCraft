//! Borrow-only input validation, completed before image allocation.
use super::{
    input::{Cell, Columns, Rows},
    profile::{self, Field, Fields},
    stored_text, variable,
};
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;
use thiserror::Error;

/// Payload-free input, resource or serialization failure for fresh creation.
#[derive(Debug, Error)]
pub enum CreateDatabaseError {
    /// The bounded multi-table operation needs one through sixteen user tables.
    #[error("creation requires between one and sixteen user tables")]
    TableLimit,
    /// Two table names collide under the same ASCII folding policy.
    #[error("creation table names must be distinct under ASCII folding")]
    DuplicateTable,
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
    #[error("creation requires between one and sixteen scalar columns")]
    ColumnLimit,
    /// This bounded profile admits at most sixty-four user rows.
    #[error("creation admits at most sixty-four rows")]
    RowLimit,
    /// A row does not contain exactly the declared number of scalar cells.
    #[error("creation row width differs from the declared columns")]
    RowWidth,
    /// The selected rows and directory do not fit the one user DATA page.
    #[error("creation rows do not fit the single user DATA page")]
    PageCapacity,
    /// A checked internal layout bound was not satisfied; no artifact is returned.
    #[error("creation layout bounds were not satisfied")]
    Layout,
    /// The requested family has no qualified bound declaration in this profile.
    #[error("creation requires a qualified native scalar declaration")]
    UnsupportedType,
    /// A cell has the wrong exact carrier or supplies NULL for Boolean.
    #[error("creation cell does not match its declared native scalar type")]
    Carrier,
    /// An explicit native variable declaration is outside this constructive profile.
    #[error("creation native declaration is outside the constructive profile")]
    Declaration,
    /// A matching carrier exceeds its capacity or differs from the exact Decimal scale.
    #[error("creation cell violates its explicit native declaration")]
    DeclaredValue,
    /// Text is valid but has no representation in the qualified stored-text profile.
    #[error("creation text has no qualified reversible native encoding")]
    TextEncoding,
    /// Cancellation, allocation or a shared resource limit rejected the operation.
    #[error(transparent)]
    Inspect(#[from] InspectError),
}

pub(super) struct Request<'a> {
    pub catalog_name: stored_text::Plan<'a>,
    pub columns: Columns<'a>,
    pub rows: Rows<'a>,
    pub row_bytes: usize,
    pub free_bytes: usize,
    pub minimum_row_bytes: usize,
    pub variable: Option<variable::Prepared<'a>>,
    fields: [Field<'a>; profile::MAX_COLUMNS],
}

impl Request<'_> {
    pub fn row_length(&self, row: usize) -> usize {
        self.variable
            .as_ref()
            .map_or(self.row_bytes, |plans| plans.row_length(row))
    }

    pub fn fields(&self) -> Fields<'_> {
        Fields::new(&self.fields[..self.columns.count()])
    }
}

pub(super) fn validate<'a>(
    table: &'a str,
    columns: Columns<'a>,
    rows: Rows<'a>,
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
    let catalog_name = stored_text::Plan::capture(table, budget, cancelled)?;
    // Every authored ASCII system name is shorter than a maximum-size user
    // name. Bound all eight rows by the larger row, including directory bytes.
    let largest_name = stored_text::Plan::ascii_literal("MSysRelationships");
    let largest_row = super::record::catalog_length(catalog_name)
        .max(super::record::catalog_length(largest_name));
    let catalog_used = largest_row
        .checked_add(size_of::<u16>())
        .and_then(|bytes| bytes.checked_mul(profile::CATALOG_OBJECT_COUNT))
        .and_then(|bytes| bytes.checked_add(super::record::DATA_HEADER_BYTES))
        .ok_or(CreateDatabaseError::PageCapacity)?;
    if catalog_used > PAGE_BYTES {
        return Err(CreateDatabaseError::PageCapacity);
    }
    if columns.count() == 0 || columns.count() > profile::MAX_COLUMNS {
        return Err(CreateDatabaseError::ColumnLimit);
    }

    // Only the filled prefix is ever exposed to the serializer. This bounded
    // stack storage owns no copied names, Values or fabricated NativeColumns.
    let mut fields = [Field {
        name: "",
        kind: 0,
        offset: None,
        width: 0,
        decimal: None,
    }; profile::MAX_COLUMNS];
    let mut fixed_bytes = 0_usize;
    for (position, field) in fields[..columns.count()].iter_mut().enumerate() {
        let column = columns.name(position);
        name(column, budget, cancelled)?;
        for prior in 0..position {
            check(budget, cancelled)?;
            compare(column, columns.name(prior), budget)?;
            if column.eq_ignore_ascii_case(columns.name(prior)) {
                return Err(CreateDatabaseError::DuplicateColumn);
            }
        }
        *field = profile::bind(column, columns.declaration(position), fixed_bytes)?;
        if field.offset.is_some() {
            fixed_bytes = fixed_bytes
                .checked_add(field.width)
                .ok_or(CreateDatabaseError::Layout)?;
        }
    }
    if rows.count() > profile::MAX_ROWS {
        return Err(CreateDatabaseError::RowLimit);
    }
    for row in 0..rows.count() {
        check(budget, cancelled)?;
        if rows.width(row) != columns.count() {
            return Err(CreateDatabaseError::RowWidth);
        }
        budget
            .charge(Resource::Steps, rows.width(row) as u64)
            .map_err(InspectError::from)?;
        if let Rows::Values(_) = rows {
            for column in 0..columns.count() {
                check(budget, cancelled)?;
                let Cell::Value(value) = rows.cell(row, column) else {
                    return Err(CreateDatabaseError::Layout);
                };
                let scalar = columns.scalar(column);
                if matches!(value, Value::Null) {
                    if scalar == ScalarType::Boolean {
                        return Err(CreateDatabaseError::Carrier);
                    }
                } else if ScalarType::of(value) != Some(scalar) {
                    return Err(CreateDatabaseError::Carrier);
                }
            }
        }
    }

    let row_bytes = size_of::<u16>()
        .checked_add(fixed_bytes)
        .and_then(|length| length.checked_add(columns.count().div_ceil(u8::BITS as usize)))
        .ok_or(CreateDatabaseError::Layout)?;
    let variables = Fields::new(&fields[..columns.count()]).variable_count();
    let variable = if variables == 0 {
        None
    } else {
        Some(variable::prepare(
            columns,
            rows,
            size_of::<u16>() + fixed_bytes,
            variables,
            budget,
            cancelled,
        )?)
    };
    let (free_bytes, minimum_row_bytes) = if let Some(prepared) = &variable {
        (prepared.free_bytes, prepared.minimum_row_bytes)
    } else {
        let used = rows
            .count()
            .checked_mul(row_bytes + size_of::<u16>())
            .and_then(|bytes| bytes.checked_add(super::record::DATA_HEADER_BYTES))
            .ok_or(CreateDatabaseError::PageCapacity)?;
        let free_bytes = PAGE_BYTES
            .checked_sub(used)
            .ok_or(CreateDatabaseError::PageCapacity)?;
        (free_bytes, row_bytes)
    };
    if super::definition::length(Fields::new(&fields[..columns.count()]), false)? > PAGE_BYTES {
        return Err(CreateDatabaseError::PageCapacity);
    }
    check(budget, cancelled)?;

    Ok(Request {
        catalog_name,
        columns,
        rows,
        row_bytes,
        free_bytes,
        minimum_row_bytes,
        variable,
        fields,
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
