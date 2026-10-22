//! Exact UNION carrier widening before duplicate removal and final ordering.
use super::Runtime;
use crate::{OutputType, QueryError};
use tablecraft_core::values::{Double, ExpressionValue, Value};
use tablecraft_schema::ScalarType;

impl Runtime<'_, '_> {
    pub(super) fn promote_union_rows(
        &mut self,
        rows: &mut [Vec<ExpressionValue>],
        types: &[OutputType],
    ) -> Result<(), QueryError> {
        for row in rows {
            self.tick()?;
            if row.len() != types.len() {
                return Err(QueryError::Width);
            }
            for (value, kind) in row.iter_mut().zip(types) {
                self.tick()?;
                let (ExpressionValue::Value(value), OutputType::Scalar { data_type, .. }) =
                    (value, kind)
                else {
                    // Dynamic columns and expression-only states retain their
                    // existing consumers; none becomes a stored NULL or scalar.
                    continue;
                };
                *value = match (data_type.scalar_type(), &*value) {
                    (ScalarType::Int16, Value::Byte(value)) => Value::Int16(i16::from(*value)),
                    (ScalarType::Int32, Value::Byte(value)) => Value::Int32(i32::from(*value)),
                    (ScalarType::Int32, Value::Int16(value)) => Value::Int32(i32::from(*value)),
                    (ScalarType::Int64, Value::Byte(value)) => Value::Int64(i64::from(*value)),
                    (ScalarType::Int64, Value::Int16(value)) => Value::Int64(i64::from(*value)),
                    (ScalarType::Int64, Value::Int32(value)) => Value::Int64(i64::from(*value)),
                    (ScalarType::Double, Value::Single(value)) => {
                        Value::Double(Double::from_float(f64::from(value.to_float())))
                    }
                    _ => continue,
                };
            }
        }
        Ok(())
    }
}
