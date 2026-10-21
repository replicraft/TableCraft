//! Evaluate each operand once; retain bounded finite doubles for centered passes.
mod spread;
use super::super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Expr, Statistic},
};
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::functions::conversions::Conversion;

impl Runtime<'_> {
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
            let value = self.eval(
                argument,
                Environment {
                    row,
                    group: None,
                    ..env
                },
            )?;
            if matches!(value, Value::Null) {
                continue;
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
