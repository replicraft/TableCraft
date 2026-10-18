use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::numeric::{self, Arithmetic};

impl Runtime<'_> {
    pub(super) fn aggregate(
        &mut self,
        kind: Aggregate,
        argument: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let group = env.group.ok_or(QueryError::Grouping)?;
        let mut count = 0_i64;
        let mut result = Value::Null;
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
                Aggregate::Sum | Aggregate::Average => {
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
                    if matches!(result, Value::Null)
                        || self.order(&value, &result)?
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
        if matches!(kind, Aggregate::Average) && count > 0 {
            return Ok(numeric::arithmetic(
                Arithmetic::Divide,
                &result,
                &Value::Int64(count),
            )?);
        }
        Ok(result)
    }
}
