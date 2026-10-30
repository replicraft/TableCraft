//! Passive field defaults for INSERT requests that supply every ordinary field.
use super::{SUPPORTED_PROPERTY_FLAG_MASK, WriteBlocker};
use crate::{
    EditError, NativeProperty, PropertyGroup, PropertyGroupKind, TableSchema,
    format::{
        data_type,
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

// This qualification does not evaluate expression syntax. The complete-row and
// companion normalizers separately reject omitted fields before serialization.
pub(super) fn qualify(
    schema: &TableSchema,
    group: &PropertyGroup,
    property: &NativeProperty,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let refused = || EditError::Unsupported(WriteBlocker::Properties);
    if group.kind() != PropertyGroupKind::Column
        || !property.name().eq_ignore_ascii_case("DefaultValue")
        || property.flags() & !SUPPORTED_PROPERTY_FLAG_MASK != 0
        || !matches!(
            property.native_type(),
            data_type::SHORT_TEXT | data_type::LONG_TEXT
        )
        || property.text().is_err()
    {
        return Err(refused());
    }

    for column in schema.columns() {
        check(budget, cancelled)?;
        if !column.name().eq_ignore_ascii_case(group.name()) {
            continue;
        }
        if column.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0
            || column.native_type() == data_type::OLE
        {
            return Err(refused());
        }

        return Ok(());
    }

    Err(refused())
}
