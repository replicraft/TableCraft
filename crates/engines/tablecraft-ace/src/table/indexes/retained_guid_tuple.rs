//! Manual GUID/I32 tuple metadata for complete readonly selected-table proof.
use super::{NativeIndex, constraints::PRIMARY_INDEX, profile::IndexProfile};
use crate::{
    InspectError, TableSchema,
    format::{
        data_type::{GUID, INT32},
        layout::{FIXED_FLAG, INDEX_KEY_COLUMNS, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Guid,
};
use tablecraft_schema::ScalarType;

// Existing profile selection resolves bounded key metadata in separate passes.
// Pay before its unmetered constructor; our own guarded loop is paid separately.
const METADATA_PASSES: usize = 8;

pub(in crate::table) fn validate_manual_guid_tuple(
    schema: &TableSchema,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns().len())
        || index.native_type() > PRIMARY_INDEX
        || index.foreign_reference().is_some()
    {
        return Err(InspectError::Unsupported);
    }
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if constraints.ignore_nulls() && constraints.primary_key() {
        return Err(InspectError::Unsupported);
    }
    let mut has_guid = false;
    for key in index.columns() {
        check(budget, cancelled)?;
        let field = schema
            .columns()
            .get(key.number())
            .ok_or(InspectError::Unsupported)?;
        let width = match (field.native_type(), field.scalar_type()) {
            (GUID, Some(ScalarType::Guid)) => {
                has_guid = true;
                Guid::BYTE_LEN
            }
            (INT32, Some(ScalarType::Int32)) => size_of::<i32>(),
            _ => return Err(InspectError::Unsupported),
        };
        if field.number() != key.number()
            || !field.fixed()
            || field.storage_length() != width
            || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.complex_type_id().is_some()
        {
            return Err(InspectError::Unsupported);
        }
    }
    if !has_guid {
        return Err(InspectError::Unsupported);
    }
    let work = index
        .columns()
        .len()
        .checked_mul(schema.columns().len())
        .and_then(|work| work.checked_add(INDEX_KEY_COLUMNS))
        .and_then(|work| work.checked_mul(METADATA_PASSES))
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, work as u64)?;
    match IndexProfile::for_check(index, schema)? {
        IndexProfile::GuidComposite(_) => {}
        _ => return Err(InspectError::Unsupported),
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
