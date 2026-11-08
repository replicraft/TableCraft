//! Reconstruct only the existing constructive declarations and inert properties.
use super::{CreatedAddColumnError, properties};
use crate::{
    CreateColumn, FirstTableDefinition, NativeColumn, NativeImage, TableCheck, format::data_type,
    header::check, inspect_table_properties,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::{DecimalConstraints, ScalarType};

pub(super) fn capture(
    image: &NativeImage,
    checked: &TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FirstTableDefinition, CreatedAddColumnError> {
    let schema = checked.schema();
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(CreatedAddColumnError::Source);
    }
    if !properties.raw_bytes().is_empty() {
        crate::properties::framing::validate(&properties, budget, cancelled)?;
    }
    let policies = properties::capture(schema, &properties, budget, cancelled)?;
    budget
        .charge_allocation(
            schema.columns().len() as u64,
            size_of::<CreateColumn<'_>>() as u64,
        )
        .map_err(crate::InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| crate::InspectError::Allocation)?;
    for (position, (field, policy)) in schema.columns().iter().zip(&policies).enumerate() {
        check(budget, cancelled)?;
        let mut column = declaration(field)?;
        if let Some(required) = policy.required {
            column = column.with_required(required);
        }
        if let Some(allow) = policy.allow_empty {
            column = column.with_allow_zero_length(allow)?;
        }
        if let Some(value) = &policy.default {
            column = column.with_default_literal(value)?;
        }
        if let Some(description) = policy.description {
            column = column.with_description(description)?;
        }
        if position == 0 {
            // Source::capture has checked one first-field primary. The complete
            // canonical reconstruction below still proves all raw native flags.
            column = column
                .with_primary_key(checked.indexes()[0].index().name())?
                .with_incrementing_autonumber()?;
        }
        columns.push(column);
    }
    Ok(FirstTableDefinition::capture(
        schema.name(),
        &columns,
        budget,
        cancelled,
    )?)
}

fn declaration(field: &NativeColumn) -> Result<CreateColumn<'_>, CreatedAddColumnError> {
    let kind = field.scalar_type().ok_or(CreatedAddColumnError::Source)?;
    Ok(match kind {
        ScalarType::Text if field.native_type() == data_type::SHORT_TEXT => {
            CreateColumn::short_text(field.name(), field.storage_length() / size_of::<u16>())?
        }
        ScalarType::Binary if field.native_type() == data_type::SHORT_BINARY => {
            CreateColumn::short_binary(field.name(), field.storage_length())?
        }
        ScalarType::Decimal => {
            let (precision, scale) = field
                .decimal_parameters()
                .ok_or(CreatedAddColumnError::Source)?;
            let constraints = DecimalConstraints::new(precision, scale)
                .map_err(|_| CreatedAddColumnError::Source)?;
            CreateColumn::decimal(field.name(), constraints)
        }
        ScalarType::Boolean
        | ScalarType::Byte
        | ScalarType::Int16
        | ScalarType::Int32
        | ScalarType::Int64
        | ScalarType::Currency
        | ScalarType::Single
        | ScalarType::Double
        | ScalarType::DateTime
        | ScalarType::Guid => CreateColumn::new(field.name(), kind),
        _ => return Err(CreatedAddColumnError::Source),
    })
}
