//! Thin wire mapping to the builder's validated single-column declarations.
use super::{FieldAddition, FieldKind, NumberSize};
use serde::Deserialize;
use serde_json::Value;
use std::num::NonZeroU8;
use tablecraft_builders::table::{
    DraftNumberSize, FieldAdditionDeclaration, TypedFieldAdditionDeclaration,
};
use tablecraft_schema::ScalarType;

pub(crate) fn declaration(arguments: &Value) -> Result<FieldAdditionDeclaration, String> {
    let request = FieldAddition::deserialize(arguments).map_err(|error| error.to_string())?;
    if !matches!(request.kind, FieldKind::ShortText)
        && (request.max_utf16_units.is_some() || request.unicode_compression.is_some())
    {
        return Err("Text width and compression apply only to Short Text addition".into());
    }
    if !matches!(request.kind, FieldKind::Number)
        && (request.number_size.is_some() || request.precision.is_some() || request.scale.is_some())
    {
        return Err("Number size and Decimal parameters apply only to Number addition".into());
    }
    let typed = match request.kind {
        FieldKind::ShortText => {
            return Ok(FieldAdditionDeclaration::ShortText {
                max_utf16_units: NonZeroU8::new(request.max_utf16_units.unwrap_or(255))
                    .ok_or("max_utf16_units must be nonzero")?,
                unicode_compression: request.unicode_compression.unwrap_or(true),
            });
        }
        FieldKind::Number => {
            if !matches!(request.number_size, Some(NumberSize::Decimal))
                && (request.precision.is_some() || request.scale.is_some())
            {
                return Err("Precision and scale require Decimal Number size".into());
            }
            let size = match request.number_size {
                None => return Ok(FieldAdditionDeclaration::NumberLongZero),
                Some(NumberSize::Byte) => DraftNumberSize::Byte,
                Some(NumberSize::Integer) => DraftNumberSize::Integer,
                Some(NumberSize::LongInteger) => DraftNumberSize::LongInteger,
                Some(NumberSize::Single) => DraftNumberSize::Single,
                Some(NumberSize::Double) => DraftNumberSize::Double,
                Some(NumberSize::Decimal) => DraftNumberSize::Decimal,
                Some(NumberSize::ReplicationId) => DraftNumberSize::ReplicationId,
            };
            if size == DraftNumberSize::Decimal {
                let defaults = DraftNumberSize::initial_decimal_constraints();
                TypedFieldAdditionDeclaration::decimal(
                    request.precision.unwrap_or(defaults.precision()),
                    request.scale.unwrap_or(defaults.scale()),
                )
            } else {
                TypedFieldAdditionDeclaration::fixed(size.scalar_type())
            }
        }
        FieldKind::LargeInteger => TypedFieldAdditionDeclaration::fixed(ScalarType::Int64),
        FieldKind::YesNo => TypedFieldAdditionDeclaration::fixed(ScalarType::Boolean),
        FieldKind::Currency => TypedFieldAdditionDeclaration::fixed(ScalarType::Currency),
        FieldKind::DateTime => TypedFieldAdditionDeclaration::fixed(ScalarType::DateTime),
    }
    .map_err(|error| error.to_string())?;
    Ok(FieldAdditionDeclaration::Typed(typed))
}

#[cfg(test)]
mod tests;
