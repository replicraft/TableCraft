//! Evaluate each operand once; retain bounded finite doubles for centered passes.
mod spread;
use super::super::{Environment, Runtime};
use crate::execution::cell::into_operator_scalar;
use crate::{
    QueryError,
    plan::{Expr, Statistic},
};
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::Conversion};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn statistic(
        &mut self,
        kind: Statistic,
        argument: &Expr,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        self.tick()?;
        self.budget
            .charge_allocation(group.len() as u64, size_of::<f64>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(group.len())
            .map_err(|_| QueryError::Allocation)?;
        let mut scale = 0_f64;
        for row in group {
            self.tick()?;
            let value = into_operator_scalar(self.expression_operand(
                argument,
                Environment {
                    row,
                    group: None,
                    group_keys: None,
                    ..env
                },
            )?)?;
            if matches!(value, Value::Null) {
                continue;
            }
            if !matches!(
                value,
                Value::Boolean(_)
                    | Value::Byte(_)
                    | Value::Int16(_)
                    | Value::Int32(_)
                    | Value::Int64(_)
                    | Value::Single(_)
                    | Value::Double(_)
                    | Value::Currency(_)
                    | Value::Decimal(_)
            ) {
                return Err(EvalError::TypeMismatch.into());
            }

            let Value::Double(value) = Conversion::Double.apply(&value)? else {
                return Err(QueryError::Unsupported);
            };
            let value = value.to_float();
            scale = scale.max(value.abs());
            values.push(value);
        }
        self.tick()?;
        let count = u32::try_from(values.len()).map_err(|_| QueryError::StructureLimit)?;
        let Some(denominator) = kind.denominator(count) else {
            return Ok(Value::Null);
        };
        let value = spread::calculate(&values, scale, denominator, kind, &mut || self.tick())?;
        self.tick()?;
        Ok(Value::Double(Double::from_float(value)))
    }
}
