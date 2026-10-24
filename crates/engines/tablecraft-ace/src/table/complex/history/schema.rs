use super::super::encoding::OBSERVED_HISTORY_EXT_FLAG;
use crate::{
    ComplexColumnLinks, ComplexColumnSchema, ComplexTableSchema, InspectError, NativeColumn,
    TableSchema,
    format::{
        data_type,
        layout::{FIXED_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const HISTORY_FIELDS: usize = 2;

pub(super) struct Columns {
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
    let memo_position = schema
        .owner()
        .columns()
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(name))
        .ok_or(InspectError::UnknownColumn)?;
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
        if !template.columns().iter().any(|field| {
            field.native_type() == data_type::LONG_TEXT
                && field.name().eq_ignore_ascii_case(memo.name())
        }) {
            continue;
        }
        if selected.is_some() {
            return Err(InspectError::Malformed);
        }
        let (value, _) = type_fields(template)?;
        if !value.name().eq_ignore_ascii_case(memo.name()) {
            return Err(InspectError::Unsupported);
        }
        let position = schema
            .owner()
            .columns()
            .iter()
            .position(|field| field.number() == column.number())
            .ok_or(InspectError::Malformed)?;
        if schema.owner().columns()[position].extended_flags() != OBSERVED_HISTORY_EXT_FLAG {
            return Err(InspectError::Unsupported);
        }
        selected = Some(position);
    }
    Ok((selected.ok_or(InspectError::Unsupported)?, memo_position))
}

pub(super) fn read(links: &ComplexColumnLinks) -> Result<Columns, InspectError> {
    qualify(links.column(), links.child_snapshot().schema())
}

pub(super) fn qualify(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
) -> Result<Columns, InspectError> {
    let (value, modified) = type_fields(column.type_table())?;
    let position = |template: &NativeColumn| {
        let (position, field) = flat
            .columns()
            .iter()
            .enumerate()
            .find(|(_, field)| field.name().eq_ignore_ascii_case(template.name()))
            .ok_or(InspectError::Malformed)?;
        if !qualified(field, template.native_type()) {
            return Err(InspectError::Unsupported);
        }
        Ok(position)
    };
    let identity = flat
        .columns()
        .iter()
        .position(|field| field.number() == column.identity_number())
        .ok_or(InspectError::Malformed)?;
    Ok(Columns {
        value: position(value)?,
        modified: position(modified)?,
        identity,
    })
}
pub(super) fn type_fields(
    template: &TableSchema,
) -> Result<(&NativeColumn, &NativeColumn), InspectError> {
    if template.declared_rows() != 0 || template.columns().len() != HISTORY_FIELDS {
        return Err(InspectError::Unsupported);
    }
    let value = template
        .columns()
        .iter()
        .find(|field| qualified(field, data_type::LONG_TEXT))
        .ok_or(InspectError::Unsupported)?;
    let modified = template
        .columns()
        .iter()
        .find(|field| qualified(field, data_type::DATE_TIME))
        .ok_or(InspectError::Unsupported)?;
    Ok((value, modified))
}

// The visible current Memo uses the ordinary Memo codec. Its Unicode
// compression marker does not change the hidden history's schema or encoding.
fn qualified_visible_memo(field: &NativeColumn) -> bool {
    qualified_storage(field, data_type::LONG_TEXT)
        && matches!(field.extended_flags(), 0 | UNICODE_COMPRESSION_FLAG)
}

fn qualified(field: &NativeColumn, kind: u8) -> bool {
    qualified_storage(field, kind) && field.extended_flags() == 0
}

fn qualified_storage(field: &NativeColumn, kind: u8) -> bool {
    let flags = UPDATABLE_FLAG | if field.fixed() { FIXED_FLAG } else { 0 };
    field.native_type() == kind
        && field.fixed() == (kind == data_type::DATE_TIME)
        && field.flags() == flags
        && field.scalar_type().is_some()
}
