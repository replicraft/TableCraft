//! Mutation authority is narrower than the read-only long-value report.
use super::super::{EditError, WriteBlocker, gates, preview};
use super::encode;
use crate::{
    InspectError, LongValueCheckStatus, NativeImage, PropertyGroupKind, TableCheck,
    TableLongValueCheck, check_table_long_values, inspect_table_properties,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Prepared {
    pub structural: TableCheck,
    pub column: usize,
    pub storage: TableLongValueCheck,
}

pub(super) fn prepare(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    preview::qualify(image)?;
    let structural = gates::before_slot(image, table, budget, cancelled)?;
    let schema = structural.schema();
    let position = schema
        .columns()
        .iter()
        .position(|candidate| candidate.name().eq_ignore_ascii_case(column))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[position];
    encode::qualify(field)?;

    if structural.indexes().iter().any(|index| {
        index
            .index()
            .columns()
            .iter()
            .any(|key| key.number() == field.number())
    }) {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    if schema.columns().iter().any(|other| {
        other.number() != field.number()
            && !other.fixed()
            && other.field.variable == field.field.variable
    }) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    let scope = image.catalog().catalog().scope();
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    if properties.groups().iter().any(|group| {
        group.kind() == PropertyGroupKind::Column
            && group.name().eq_ignore_ascii_case(field.name())
            && !group.properties().is_empty()
    }) {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }

    let storage = check_table_long_values(&mut image.reader(), scope, table, budget, cancelled)?;
    let selected = storage
        .columns()
        .iter()
        .find(|candidate| candidate.column_number() == field.number())
        .ok_or(InspectError::Malformed)?;
    if selected.status() != LongValueCheckStatus::Checked
        || selected.shared_references() != 0
        || selected.shared_owned_pages() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    Ok(Prepared {
        structural,
        column: position,
        storage,
    })
}
