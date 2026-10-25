//! UNION result conversion before duplicate removal and final ordering.
mod policies;
use super::Runtime;
use crate::execution::cell::CellValue;
use crate::{OutputType, QueryError, source::TextPolicy};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::{conversions::Conversion, text::TextFunction};
use tablecraft_schema::ScalarType;

impl Runtime<'_, '_> {
    pub(super) fn promote_union_rows(
        &mut self,
        rows: &mut [Vec<CellValue>],
        types: &[OutputType],
    ) -> Result<(), QueryError> {
        for row in rows {
            self.tick()?;
            if row.len() != types.len() {
                return Err(QueryError::Width);
            }
            for (cell, kind) in row.iter_mut().zip(types) {
                self.tick()?;
                // UNION declaration rules are not qualified; row presence or a
                // selected duplicate must not manufacture scalar provenance.
                cell.declaration = None;
                let (ExpressionValue::Value(value), OutputType::Scalar { data_type, .. }) =
                    (&mut cell.value, kind)
                else {
                    // Dynamic columns and expression-only states retain their
                    // existing consumers; none becomes a stored NULL or scalar.
                    continue;
                };
                *value = match (data_type.scalar_type(), &*value) {
                    (
                        ScalarType::Text,
                        Value::Boolean(_)
                        | Value::Byte(_)
                        | Value::Int16(_)
                        | Value::Int32(_)
                        | Value::Int64(_)
                        | Value::Single(_)
                        | Value::Double(_)
                        | Value::Currency(_)
                        | Value::Decimal(_),
                    ) => {
                        let converted = self
                            .evaluator()
                            .apply_text_function(TextFunction::CStr, std::slice::from_ref(value))?;
                        // A rendered number has fresh CStr lineage. Its numeric
                        // source may carry policies from unselected String arms;
                        // those fields did not contribute to the new String.
                        cell.policy = TextPolicy::Default;
                        cell.policy_complete = true;
                        converted
                    }
                    (ScalarType::Int16, Value::Boolean(value)) => {
                        Value::Int16(if *value { -1 } else { 0 })
                    }
                    (ScalarType::Int16, Value::Byte(value)) => Value::Int16(i16::from(*value)),
                    (ScalarType::Int32, Value::Boolean(value)) => {
                        Value::Int32(if *value { -1 } else { 0 })
                    }
                    (ScalarType::Int32, Value::Byte(value)) => Value::Int32(i32::from(*value)),
                    (ScalarType::Int32, Value::Int16(value)) => Value::Int32(i32::from(*value)),
                    (ScalarType::Int64, Value::Boolean(value)) => {
                        Value::Int64(if *value { -1 } else { 0 })
                    }
                    (ScalarType::Int64, Value::Byte(value)) => Value::Int64(i64::from(*value)),
                    (ScalarType::Int64, Value::Int16(value)) => Value::Int64(i64::from(*value)),
                    (ScalarType::Int64, Value::Int32(value)) => Value::Int64(i64::from(*value)),
                    (
                        ScalarType::Double,
                        Value::Boolean(_)
                        | Value::Byte(_)
                        | Value::Int16(_)
                        | Value::Int32(_)
                        | Value::Int64(_)
                        | Value::Single(_)
                        | Value::Currency(_)
                        | Value::Decimal(_),
                    ) => Conversion::Double.apply(value)?,
                    (
                        ScalarType::Currency,
                        Value::Boolean(_) | Value::Byte(_) | Value::Int16(_) | Value::Int32(_),
                    ) => Conversion::Currency.apply(value)?,
                    (
                        ScalarType::Decimal,
                        Value::Boolean(_)
                        | Value::Byte(_)
                        | Value::Int16(_)
                        | Value::Int32(_)
                        | Value::Int64(_)
                        | Value::Currency(_),
                    ) => Conversion::Decimal.apply(value)?,
                    _ => continue,
                };
            }
        }
        Ok(())
    }
}
