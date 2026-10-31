//! Exact declarations preserved by a Boolean-only, schema-stable operation.
use crate::{
    EditError, InspectError, NativeColumn, NativeProperty, PropertyGroupKind, TableProperties,
    TableSchema, WriteBlocker,
    format::{data_type, layout::CALCULATED_EXT_MASK},
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DDL_FLAG: u8 = 1;
const MAX_NAME_MAP_BYTES: usize = 1024 * 1024;

pub(super) fn qualify(
    schema: &TableSchema,
    properties: &TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for group in properties.groups() {
        check(budget, cancelled)?;
        let field = match group.kind() {
            PropertyGroupKind::Table if group.name().is_empty() => None,
            PropertyGroupKind::Column => {
                let mut selected = None;
                for field in schema.columns() {
                    check(budget, cancelled)?;
                    budget
                        .charge(
                            Resource::Steps,
                            field.name().len().max(group.name().len()) as u64,
                        )
                        .map_err(InspectError::from)?;
                    if field.name().eq_ignore_ascii_case(group.name())
                        && selected.replace(field).is_some()
                    {
                        return Err(refused());
                    }
                }
                Some(selected.ok_or_else(refused)?)
            }
            _ => return Err(refused()),
        };
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (property.name().len() + property.raw_value().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if property.flags() & !DDL_FLAG != 0 {
                return Err(refused());
            }
            if let Some(field) = field {
                column(field, property)?;
            } else {
                table(property)?;
                if property.name().eq_ignore_ascii_case("NameMap") {
                    super::namemap::qualify(schema, properties, property, budget, cancelled)?;
                }
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

fn table(property: &NativeProperty) -> Result<(), EditError> {
    let name = property.name();
    let valid = if name.eq_ignore_ascii_case("GUID") {
        typed(property, data_type::SHORT_BINARY, 16) && property.flags() == DDL_FLAG
    } else if name.eq_ignore_ascii_case("NameMap") {
        property.native_type() == data_type::OLE
            && property.flags() == 0
            && property.raw_value().len() <= MAX_NAME_MAP_BYTES
    } else if ["FCMinReadVer", "FCMinWriteVer", "FCMinDesignVer"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && version(property)
    } else if [
        "BackTint",
        "BackShade",
        "AlternateBackTint",
        "AlternateBackShade",
    ]
    .iter()
    .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && typed(property, data_type::SINGLE, size_of::<f32>())
    } else if [
        "ThemeFontIndex",
        "AlternateBackThemeColorIndex",
        "DatasheetGridlinesThemeColorIndex",
        "DatasheetForeThemeColorIndex",
    ]
    .iter()
    .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && typed(property, data_type::INT32, size_of::<i32>())
    } else if name.eq_ignore_ascii_case("Orientation") {
        property.flags() == DDL_FLAG && typed(property, data_type::BYTE, size_of::<u8>())
    } else if name.eq_ignore_ascii_case("PublishToWeb") {
        property.flags() == 0 && property.byte().is_ok_and(|value| value == 0)
    } else if ["DefaultView", "DisplayViewsOnSharePointSite"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && typed(property, data_type::BYTE, size_of::<u8>())
    } else if name.eq_ignore_ascii_case("ReadOnlyWhenDisconnected") {
        property.flags() == 0 && property.boolean().is_ok_and(|value| !value)
    } else if [
        "OrderByOn",
        "FilterOnLoad",
        "OrderByOnLoad",
        "TotalsRow",
        "HideNewField",
    ]
    .iter()
    .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && ui_boolean(property)
    } else if ["Caption", "Description", "ValidationText", "ValidationRule"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        text(property)
    } else {
        false
    };
    if !valid {
        return Err(refused());
    }
    Ok(())
}

fn column(field: &NativeColumn, property: &NativeProperty) -> Result<(), EditError> {
    let name = property.name();
    let ordinary = field.scalar_type().is_some();
    let calculated = field.extended_flags() & CALCULATED_EXT_MASK != 0;
    let valid = if name.eq_ignore_ascii_case("GUID") {
        property.flags() == DDL_FLAG && typed(property, data_type::SHORT_BINARY, 16)
    } else if ["ColumnWidth", "ColumnOrder"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && ui_integer(property)
    } else if name.eq_ignore_ascii_case("ColumnHidden") {
        property.flags() == 0 && ui_boolean(property)
    } else if name.eq_ignore_ascii_case("TextAlign") {
        property.flags() == 0 && typed(property, data_type::BYTE, size_of::<u8>())
    } else if ["AggregateType", "CurrencyLCID"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == 0 && typed(property, data_type::INT32, size_of::<i32>())
    } else if ["DisplayControl", "ShowDatePicker"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == DDL_FLAG && typed(property, data_type::INT16, size_of::<i16>())
    } else if ["IMEMode", "IMESentenceMode", "DecimalPlaces"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        property.flags() == DDL_FLAG && typed(property, data_type::BYTE, size_of::<u8>())
    } else if name.eq_ignore_ascii_case("Format") {
        property.flags() == DDL_FLAG && text(property)
    } else if name.eq_ignore_ascii_case("ResultType") {
        property.flags() == DDL_FLAG
            && property.byte().is_ok_and(|kind| {
                if calculated {
                    kind == field.native_type()
                } else {
                    kind == 0
                }
            })
    } else if name.eq_ignore_ascii_case("Expression") {
        calculated && property.flags() == DDL_FLAG && text(property)
    } else if name.eq_ignore_ascii_case("AppendOnly") {
        ordinary
            && field.native_type() == data_type::LONG_TEXT
            && property.flags() == DDL_FLAG
            && property.boolean().is_ok_and(|value| !value)
    } else if name.eq_ignore_ascii_case("TextFormat") {
        ordinary
            && field.native_type() == data_type::LONG_TEXT
            && property.flags() == DDL_FLAG
            && property.byte().is_ok_and(|value| value == 0)
    } else if name.eq_ignore_ascii_case("UnicodeCompression") {
        ordinary
            && matches!(
                field.native_type(),
                data_type::SHORT_TEXT | data_type::LONG_TEXT
            )
            && property.flags() == DDL_FLAG
            && property.boolean().is_ok()
    } else if name.eq_ignore_ascii_case("Required") {
        ordinary && property.boolean().is_ok()
    } else if name.eq_ignore_ascii_case("AllowZeroLength") {
        ordinary
            && matches!(
                field.native_type(),
                data_type::SHORT_TEXT | data_type::LONG_TEXT
            )
            && property.boolean().is_ok()
    } else if ["DefaultValue", "ValidationRule"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        ordinary && text(property)
    } else if ["Caption", "Description", "ValidationText"]
        .iter()
        .any(|key| key.eq_ignore_ascii_case(name))
    {
        text(property)
    } else {
        false
    };
    if !valid {
        return Err(refused());
    }
    Ok(())
}

// Access-defined Datasheet dimensions retain the INT16 tag even when their
// native property payload is a signed/zero extended four-byte representation.
// This reader grants only unchanged display metadata in a Boolean row preview.
fn ui_integer(property: &NativeProperty) -> bool {
    if property.native_type() != data_type::INT16 {
        return false;
    }
    match property.raw_value() {
        [_, _] => true,
        raw @ [low, high, _, _] => {
            i32::from(i16::from_le_bytes([*low, *high]))
                .to_le_bytes()
                .as_slice()
                == raw
        }
        _ => false,
    }
}

fn typed(property: &NativeProperty, kind: u8, width: usize) -> bool {
    property.native_type() == kind && property.raw_value().len() == width
}

fn text(property: &NativeProperty) -> bool {
    matches!(
        property.native_type(),
        data_type::SHORT_TEXT | data_type::LONG_TEXT
    ) && property.text().is_ok()
}

fn ui_boolean(property: &NativeProperty) -> bool {
    property.native_type() == data_type::BOOLEAN
        && (property.raw_value().len() == size_of::<u8>()
            || matches!(property.raw_value(), [0, 0, 0, 0] | [1, 0, 0, 0]))
}

fn version(property: &NativeProperty) -> bool {
    if property.native_type() != data_type::SHORT_TEXT {
        return false;
    }
    let Ok(text) = property.text() else {
        return false;
    };
    if text.len() > 64 {
        return false;
    }
    let mut parts = text.split('.');
    parts.next() == Some("16")
        && (0..3).all(|_| {
            parts.next().is_some_and(|part| {
                !part.is_empty()
                    && part.bytes().all(|byte| byte.is_ascii_digit())
                    && part.parse::<u32>().is_ok()
            })
        })
        && parts.next().is_none()
}

fn refused() -> EditError {
    EditError::Unsupported(WriteBlocker::Properties)
}
