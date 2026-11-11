//! Exact authoring type-default values shared by table and saved-field creation.
use crate::{Error, editing::EditError};
use tablecraft_core::values::{Currency, Decimal, Double, Single, Value};
use tablecraft_schema::{ScalarType, TypeDescriptor};

pub(super) fn for_type(declaration: TypeDescriptor) -> Result<Option<Value>, Error> {
    Ok(match declaration.scalar_type() {
        ScalarType::Boolean => Some(Value::Boolean(false)),
        ScalarType::Byte => Some(Value::Byte(0)),
        ScalarType::Int16 => Some(Value::Int16(0)),
        ScalarType::Int32 => Some(Value::Int32(0)),
        ScalarType::Int64 => Some(Value::Int64(0)),
        ScalarType::Currency => Some(Value::Currency(Currency::from_scaled(0))),
        ScalarType::Single => Some(Value::Single(Single::from_bits(0))),
        ScalarType::Double => Some(Value::Double(Double::from_bits(0))),
        ScalarType::Decimal => Some(Value::Decimal(Decimal::new(
            0,
            declaration
                .decimal_constraints()
                .ok_or(EditError::InvalidInput)?
                .scale(),
        )?)),
        ScalarType::DateTime | ScalarType::Guid | ScalarType::Text => None,
        ScalarType::Binary | ScalarType::DateTimeExtended => {
            return Err(EditError::Unavailable.into());
        }
    })
}
