//! Constructor field facts without name bindings, programs or runtime evaluation.
use super::super::TableRenameError;
use crate::{
    InspectError, NativeImage, PropertyGroupKind, TableProperties, TableSchema, format::data_type,
    header::check, inspect_table_properties,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const DDL_FLAG: u8 = 1;
const DESCRIPTION_UNITS: usize = 255;

pub(super) fn qualify(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableProperties, TableRenameError> {
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(TableRenameError::Source);
    }
    if properties.raw_bytes().is_empty() {
        if !properties.groups().is_empty() {
            return Err(TableRenameError::Dependency);
        }
        return Ok(properties);
    }
    crate::properties::framing::validate(&properties, budget, cancelled)?;
    for group in properties.groups() {
        check(budget, cancelled)?;
        if group.kind() != PropertyGroupKind::Column {
            return Err(TableRenameError::Dependency);
        }
        budget
            .charge(Resource::Steps, schema.columns().len() as u64)
            .map_err(InspectError::from)?;
        let column = schema
            .columns()
            .iter()
            .find(|column| column.name() == group.name())
            .ok_or(TableRenameError::Dependency)?;
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (property.name().len() + property.raw_value().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if property.flags() != DDL_FLAG {
                return Err(TableRenameError::Dependency);
            }
            let qualified = match property.name() {
                "Required" => {
                    property.native_type() == data_type::BOOLEAN
                        && matches!(property.raw_value(), [0] | [255])
                }
                "AllowZeroLength" => {
                    column.scalar_type() == Some(ScalarType::Text)
                        && property.native_type() == data_type::BOOLEAN
                        && matches!(property.raw_value(), [0] | [255])
                }
                "DefaultValue" => {
                    property.native_type() == data_type::LONG_TEXT
                        && property.raw_value() == [b'0', 0]
                }
                "Description" => {
                    property.native_type() == data_type::SHORT_TEXT
                        && property.text()?.encode_utf16().count() <= DESCRIPTION_UNITS
                }
                _ => false,
            };
            if !qualified {
                return Err(TableRenameError::Dependency);
            }
        }
    }
    check(budget, cancelled)?;
    Ok(properties)
}
