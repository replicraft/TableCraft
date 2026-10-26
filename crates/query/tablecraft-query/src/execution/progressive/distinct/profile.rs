//! Admission uses active output descriptors; runtime cells never invent a profile.
use crate::{
    OutputType, QueryError,
    execution::{Runtime, cell::CellValue},
    plan::ReadPlan,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_schema::ScalarType;

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn progressive_distinct_profile(
        &mut self,
        read: &ReadPlan,
    ) -> Result<(), QueryError> {
        for output in read.active_types(self.options.declared_integral) {
            self.tick()?;
            match output {
                OutputType::Null => {}
                OutputType::Scalar { data_type, .. }
                    if matches!(
                        data_type.scalar_type(),
                        ScalarType::Boolean
                            | ScalarType::Byte
                            | ScalarType::Int16
                            | ScalarType::Int32
                            | ScalarType::Int64
                            | ScalarType::Currency
                            | ScalarType::Decimal
                    ) => {}
                _ => return Err(QueryError::Unsupported),
            }
        }
        Ok(())
    }

    pub(super) fn validate_distinct_row(
        &mut self,
        cells: &[CellValue],
        types: &[OutputType],
    ) -> Result<(), QueryError> {
        if cells.len() != types.len() {
            return Err(QueryError::Width);
        }
        self.validate_key_row(cells)?;
        for (cell, output) in cells.iter().zip(types) {
            self.tick()?;
            if matches!(
                cell.value,
                ExpressionValue::Value(Value::Null) | ExpressionValue::Error(_)
            ) {
                // Standard Error data is not a successful scalar subtype. Keep
                // the existing SQL key comparator's Error/non-Error decisions.
                continue;
            }
            let valid = match output {
                OutputType::Scalar { data_type, .. } => matches!(
                    (data_type.scalar_type(), &cell.value),
                    (
                        ScalarType::Boolean,
                        ExpressionValue::Value(Value::Boolean(_))
                    ) | (ScalarType::Byte, ExpressionValue::Value(Value::Byte(_)))
                        | (ScalarType::Int16, ExpressionValue::Value(Value::Int16(_)))
                        | (ScalarType::Int32, ExpressionValue::Value(Value::Int32(_)))
                        | (ScalarType::Int64, ExpressionValue::Value(Value::Int64(_)))
                        | (
                            ScalarType::Currency,
                            ExpressionValue::Value(Value::Currency(_))
                        )
                        | (
                            ScalarType::Decimal,
                            ExpressionValue::Value(Value::Decimal(_))
                        )
                ),
                OutputType::Null | OutputType::Multivalue { .. } | OutputType::Dynamic => false,
            };
            if !valid {
                return Err(QueryError::Source);
            }
        }
        Ok(())
    }
}
