//! A metadata gate, not a source of equality or an observed-carrier guess.
use crate::{
    OutputType, QueryError,
    execution::{Runtime, cell::CellValue},
    plan::{Aggregate, Expr, ProfiledType},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_schema::ScalarType;

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn progressive_text_profile(
        &mut self,
        types: &[ProfiledType],
    ) -> Result<bool, QueryError> {
        let mut text = false;
        for types in types {
            self.tick()?;
            let output = if self.options.declared_integral {
                types.declared
            } else {
                types.materialized
            };
            match output {
                OutputType::Null => {}
                OutputType::Scalar { data_type, .. } => match data_type.scalar_type() {
                    ScalarType::Text => text = true,
                    ScalarType::Boolean
                    | ScalarType::Byte
                    | ScalarType::Int16
                    | ScalarType::Int32
                    | ScalarType::Int64
                    | ScalarType::Currency
                    | ScalarType::Decimal => {}
                    _ => return Ok(false),
                },
                OutputType::Dynamic => return Ok(false),
            }
        }
        Ok(text)
    }

    pub(super) fn validate_text_group_keys(
        &mut self,
        keys: &[CellValue],
        types: &[ProfiledType],
    ) -> Result<(), QueryError> {
        if keys.len() != types.len() {
            return Err(QueryError::Source);
        }
        self.validate_key_row(keys)?;
        for (cell, types) in keys.iter().zip(types) {
            self.tick()?;
            if matches!(
                cell.value,
                ExpressionValue::Value(Value::Null) | ExpressionValue::Error(_)
            ) {
                // Standard Error data passed key-state validation above. It is
                // not a successful scalar carrier; the existing SQL pair
                // comparator still owns Error/non-Error refusal.
                continue;
            }
            let output = if self.options.declared_integral {
                types.declared
            } else {
                types.materialized
            };
            let valid = match output {
                OutputType::Scalar { data_type, .. } => matches!(
                    (data_type.scalar_type(), &cell.value),
                    (
                        ScalarType::Text,
                        ExpressionValue::StringBytes(_) | ExpressionValue::Value(Value::Text(_))
                    ) | (
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
                OutputType::Null | OutputType::Dynamic => false,
            };
            if !valid {
                return Err(QueryError::Source);
            }
        }
        Ok(())
    }

    pub(super) fn text_group_counts_only(
        &mut self,
        expressions: &[&Expr],
    ) -> Result<bool, QueryError> {
        let mut count_only = true;
        for expression in expressions {
            self.tick()?;
            if !matches!(expression, Expr::Aggregate(Aggregate::Count, None)) {
                count_only = false;
            }
        }
        Ok(count_only)
    }
}
