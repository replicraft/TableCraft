//! Qualified display declarations are retained without rendering stored values.
use super::{SUPPORTED_PROPERTY_FLAG_MASK, WriteBlocker};
use crate::{
    EditError, InspectError, NativeProperty, PropertyGroup, PropertyGroupKind, TableSchema,
    format::data_type, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_NUMERIC_DECIMAL_PLACES: u8 = 15;
const AUTOMATIC_NUMERIC_DECIMAL_PLACES: u8 = u8::MAX;

pub(super) fn requested(name: &str) -> bool {
    name.eq_ignore_ascii_case("Format") || name.eq_ignore_ascii_case("DecimalPlaces")
}

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
        || property.flags() & !SUPPORTED_PROPERTY_FLAG_MASK != 0
        || !requested(property.name())
    {
        return Err(refused());
    }

    for column in schema.columns() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                column.name().len().max(group.name().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if !column.name().eq_ignore_ascii_case(group.name()) {
            continue;
        }

        let numeric = matches!(
            column.native_type(),
            data_type::BYTE
                | data_type::INT16
                | data_type::INT32
                | data_type::INT64
                | data_type::CURRENCY
                | data_type::SINGLE
                | data_type::DOUBLE
                | data_type::NUMERIC
        );
        if property.name().eq_ignore_ascii_case("Format") {
            // This first profile covers the scalar fields in the source controls.
            // Memo and Extended Date/Time remain separate qualification work.
            if !numeric
                && !matches!(
                    column.native_type(),
                    data_type::BOOLEAN | data_type::SHORT_TEXT | data_type::DATE_TIME
                )
            {
                return Err(refused());
            }
            if property.native_type() != data_type::SHORT_TEXT || property.text().is_err() {
                return Err(refused());
            }
        } else {
            // Extended Date/Time's similarly named precision property is not the
            // numeric display-only declaration qualified by this consumer.
            if !numeric
                || property.native_type() != data_type::BYTE
                || !property.byte().is_ok_and(|value| {
                    value <= MAX_NUMERIC_DECIMAL_PLACES || value == AUTOMATIC_NUMERIC_DECIMAL_PLACES
                })
            {
                return Err(refused());
            }
        }

        return Ok(());
    }

    Err(refused())
}
