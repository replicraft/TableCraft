use super::super::encoding::OBSERVED_HISTORY_EXT_FLAG;
use crate::{
    ComplexColumnLinks, ComplexColumnSchema, ComplexTableSchema, InspectError, NativeColumn,
    TableSchema,
    format::{
        data_type,
        layout::{FIXED_FLAG, HYPERLINK_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const HISTORY_FIELDS: usize = 2;

pub(in crate::table::complex) struct Columns {
    pub value: usize,
    pub modified: usize,
    pub identity: usize,
}

pub(in crate::table::complex) fn select(
    schema: &ComplexTableSchema,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), InspectError> {
    let mut memo_position = None;
    for (position, field) in schema.owner().columns().iter().enumerate() {
        check(budget, cancelled)?;
        if names_equal(field.name(), name, budget, cancelled)? {
            memo_position = Some(position);
            break;
        }
    }
    let memo_position = memo_position.ok_or(InspectError::UnknownColumn)?;
    let memo = &schema.owner().columns()[memo_position];
    if !qualified_visible_memo(memo) {
        return Err(InspectError::Unsupported);
    }
    let mut selected = None;
    for column in schema.columns() {
        check(budget, cancelled)?;
        let template = column.type_table();
        budget.charge(
            Resource::Steps,
            (template.columns().len() + schema.owner().columns().len()) as u64,
        )?;
        let mut matches = false;
        for field in template.columns() {
            check(budget, cancelled)?;
            if field.native_type() == data_type::LONG_TEXT
                && names_equal(field.name(), memo.name(), budget, cancelled)?
            {
                matches = true;
                break;
            }
        }
        if !matches {
            continue;
        }
        if selected.is_some() {
            return Err(InspectError::Malformed);
        }
        let (value, _) = type_fields(template, budget, cancelled)?;
        if !names_equal(value.name(), memo.name(), budget, cancelled)? {
            return Err(InspectError::Unsupported);
        }
        let mut position = None;
        for (ordinal, field) in schema.owner().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if field.number() == column.number() {
                position = Some(ordinal);
                break;
            }
        }
        let position = position.ok_or(InspectError::Malformed)?;
        if schema.owner().columns()[position].extended_flags() != OBSERVED_HISTORY_EXT_FLAG {
            return Err(InspectError::Unsupported);
        }
        selected = Some(position);
    }
    Ok((selected.ok_or(InspectError::Unsupported)?, memo_position))
}

pub(super) fn read(
    links: &ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    qualify(
        links.column(),
        links.child_snapshot().schema(),
        budget,
        cancelled,
    )
}

pub(in crate::table::complex) fn qualify(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Columns, InspectError> {
    let (value, modified) = type_fields(column.type_table(), budget, cancelled)?;
    // Retain original error precedence: find the child identity before
    // qualifying the two flat member declarations.
    let mut identity = None;
    for (position, field) in flat.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() == column.identity_number() {
            identity = Some(position);
            break;
        }
    }
    let identity = identity.ok_or(InspectError::Malformed)?;
    let mut position = |template: &NativeColumn| {
        for (position, field) in flat.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if names_equal(field.name(), template.name(), budget, cancelled)? {
                if !qualified(field, template.native_type()) {
                    return Err(InspectError::Unsupported);
                }
                return Ok(position);
            }
        }
        Err(InspectError::Malformed)
    };
    Ok(Columns {
        value: position(value)?,
        modified: position(modified)?,
        identity,
    })
}

pub(super) fn type_fields<'a>(
    template: &'a TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(&'a NativeColumn, &'a NativeColumn), InspectError> {
    if template.declared_rows() != 0 || template.columns().len() != HISTORY_FIELDS {
        return Err(InspectError::Unsupported);
    }
    let mut value = None;
    for field in template.columns() {
        check(budget, cancelled)?;
        if qualified(field, data_type::LONG_TEXT) {
            value = Some(field);
            break;
        }
    }
    let value = value.ok_or(InspectError::Unsupported)?;
    let mut modified = None;
    for field in template.columns() {
        check(budget, cancelled)?;
        if qualified(field, data_type::DATE_TIME) {
            modified = Some(field);
            break;
        }
    }
    Ok((value, modified.ok_or(InspectError::Unsupported)?))
}

fn names_equal(
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    check(budget, cancelled)?;
    let work = left
        .len()
        .checked_add(right.len())
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, work as u64)?;
    Ok(left.eq_ignore_ascii_case(right))
}

// The visible current Memo uses its ordinary scalar codec. Known hyperlink
// and Unicode-compression flags do not change the hidden history encoding.
// Hyperlink components and rich-text properties remain uninterpreted data.
fn qualified_visible_memo(field: &NativeColumn) -> bool {
    field.native_type() == data_type::LONG_TEXT
        && !field.fixed()
        && field.flags() & !HYPERLINK_FLAG == UPDATABLE_FLAG
        && matches!(field.extended_flags(), 0 | UNICODE_COMPRESSION_FLAG)
        && field.scalar_type().is_some()
}

fn qualified(field: &NativeColumn, kind: u8) -> bool {
    let flags = UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 };
    field.native_type() == kind
        && field.fixed() == (kind == data_type::DATE_TIME)
        && field.flags() == flags
        && field.extended_flags() == 0
        && field.scalar_type().is_some()
}
