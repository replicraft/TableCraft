//! Number field sizes select exact carriers; defaults never infer them from text.
use crate::{Error, editing::EditError};
use tablecraft_builders::table::{DraftNumberSize, SnapshotField};
use tablecraft_core::values::{Decimal, Double, Single, Value};
use tablecraft_schema::ScalarType;

pub(super) fn scalar(size: DraftNumberSize) -> ScalarType {
    match size {
        DraftNumberSize::Byte => ScalarType::Byte,
        DraftNumberSize::Integer => ScalarType::Int16,
        DraftNumberSize::LongInteger => ScalarType::Int32,
        DraftNumberSize::Single => ScalarType::Single,
        DraftNumberSize::Double => ScalarType::Double,
        DraftNumberSize::Decimal => ScalarType::Decimal,
        DraftNumberSize::ReplicationId => ScalarType::Guid,
    }
}

pub(super) fn default(field: &SnapshotField) -> Result<Option<Value>, Error> {
    Ok(match field.number_size().ok_or(EditError::Unavailable)? {
        DraftNumberSize::Byte => Some(Value::Byte(0)),
        DraftNumberSize::Integer => Some(Value::Int16(0)),
        DraftNumberSize::LongInteger => Some(Value::Int32(0)),
        DraftNumberSize::Single => Some(Value::Single(Single::from_bits(0))),
        DraftNumberSize::Double => Some(Value::Double(Double::from_bits(0))),
        DraftNumberSize::Decimal => Some(Value::Decimal(Decimal::new(
            0,
            field
                .decimal_constraints()
                .ok_or(EditError::Unavailable)?
                .scale(),
        )?)),
        DraftNumberSize::ReplicationId => None,
    })
}
