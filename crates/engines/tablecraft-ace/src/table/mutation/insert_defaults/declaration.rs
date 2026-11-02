//! Actual per-column default declaration identity, shared by normal projections.
use crate::{
    EditError, InspectError, NativeColumn, NativeProperty, PropertyGroupKind, TableProperties,
    WriteBlocker,
    format::{
        data_type::OLE,
        layout::{AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table::mutation) fn capture<'a>(
    column: &NativeColumn,
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<&'a NativeProperty>, EditError> {
    let mut selected = None;
    for group in properties.groups() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (group.name().len() + column.name().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if group.kind() != PropertyGroupKind::Column
            || !group.name().eq_ignore_ascii_case(column.name())
        {
            continue;
        }
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, property.name().len() as u64)
                .map_err(InspectError::from)?;
            if property.name().eq_ignore_ascii_case("DefaultValue")
                && selected.replace(property).is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::Properties));
            }
        }
    }
    if selected.is_some()
        && (column.native_type() == OLE
            || column.flags() & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG) != 0)
    {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    Ok(selected)
}
