//! Evaluate each operand once; retain bounded finite doubles for centered passes.
mod fixed;
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
        let mut fixed_scale = 0_f64;
        let mut fixed_anchor = None;
        let mut fixed_values: Option<Vec<f64>> = None;
        let mut floating = false;
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

            let Value::Double(converted) = Conversion::Double.apply(&value)? else {
                return Err(QueryError::Unsupported);
            };
            if !floating {
                if let Some(sample) = fixed::sample(&value)? {
                    let anchor = *fixed_anchor.get_or_insert(sample);
                    let shifted =
                        fixed::offset(sample, anchor, self.budget, self.options.cancelled)?;
                    if fixed_values.is_none() {
                        self.budget
                            .charge_allocation(group.len() as u64, size_of::<f64>() as u64)?;
                        let mut retained = Vec::new();
                        retained
                            .try_reserve_exact(group.len())
                            .map_err(|_| QueryError::Allocation)?;
                        fixed_values = Some(retained);
                    }
                    fixed_scale = fixed_scale.max(shifted.abs());
                    fixed_values
                        .as_mut()
                        .ok_or(QueryError::Source)?
                        .push(shifted);
                } else {
                    // The original Double lane was retained in parallel. Mixed
                    // floating groups keep their existing conversion policy;
                    // no fixed sample is reconstructed or evaluated again.
                    floating = true;
                    fixed_values = None;
                }
            }
            let value = converted.to_float();
            scale = scale.max(value.abs());
            values.push(value);
        }
        self.tick()?;
        let count = u32::try_from(values.len()).map_err(|_| QueryError::StructureLimit)?;
        let Some(denominator) = kind.denominator(count) else {
            return Ok(Value::Null);
        };
        let (values, scale) = match fixed_values {
            Some(values) => (values, fixed_scale),
            None => (values, scale),
        };
        let value = spread::calculate(&values, scale, denominator, kind, &mut || self.tick())?;
        self.tick()?;
        Ok(Value::Double(Double::from_float(value)))
    }
}
