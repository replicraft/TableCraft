pub(super) mod average;
mod encounter;
mod statistics;
use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    functions::conversions::errors::into_scalar,
    numeric::{self, Arithmetic},
};

impl Runtime<'_, '_> {
    pub(super) fn aggregate(
        &mut self,
        kind: Aggregate,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        if let Aggregate::Statistic(kind) = kind {
            return self.statistic(kind, argument.ok_or(QueryError::Unsupported)?, env);
        }
        if matches!(kind, Aggregate::First | Aggregate::Last) {
            let result = self.encounter_aggregate(
                argument.ok_or(QueryError::Unsupported)?,
                env,
                matches!(kind, Aggregate::Last),
            )?;
            return Ok(into_scalar(result)?);
        }
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;
        let mut result = Value::Null;
        let mut mean = average::Mean::default();
        for row in group {
            self.tick()?;
            let value = match argument {
                Some(e) => self.eval(
                    e,
                    Environment {
                        row,
                        group: None,
                        ..env
                    },
                )?,
                None => Value::Int64(1),
            };
            if matches!(value, Value::Null) {
                continue;
            }
            count = count
                .checked_add(1)
                .ok_or(tablecraft_expressions::EvalError::Overflow)?;
            match kind {
                Aggregate::Count => {}
                Aggregate::Statistic(_) => unreachable!("handled by bounded statistical passes"),
                Aggregate::First | Aggregate::Last => unreachable!("handled before NULL filtering"),
                Aggregate::Average => mean.push(
                    &value,
                    self.budget,
                    self.options.cancelled,
                    &mut self.mean_scratch,
                )?,
                Aggregate::Sum => {
                    let zero = Value::Int64(0);
                    result = numeric::arithmetic(
                        Arithmetic::Add,
                        if matches!(result, Value::Null) {
                            &zero
                        } else {
                            &result
                        },
                        &value,
                    )?;
                }
                Aggregate::Minimum | Aggregate::Maximum => {
                    let policy = if matches!((&value, &result), (Value::Text(_), Value::Text(_))) {
                        argument
                            .map(|expr| self.text_policy(expr))
                            .transpose()?
                            .unwrap_or(crate::source::TextPolicy::Default)
                    } else {
                        crate::source::TextPolicy::Default
                    };
                    if matches!(result, Value::Null)
                        || self.order(&value, &result, policy)?
                            == if matches!(kind, Aggregate::Minimum) {
                                Ordering::Less
                            } else {
                                Ordering::Greater
                            }
                    {
                        result = value;
                    }
                }
            }
        }
        if matches!(kind, Aggregate::Count) {
            return Ok(Value::Int64(count));
        }
        if matches!(kind, Aggregate::Average) {
            return mean.finish(
                count,
                self.budget,
                self.options.cancelled,
                &mut self.mean_scratch,
            );
        }
        Ok(result)
    }
}
