//! Wider exact totals for means; source carriers and SUM retain their own rules.
mod binary;
use crate::QueryError;
pub(in crate::execution) use binary::Scratch;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_expressions::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{self, Arithmetic},
};

pub(super) struct Mean {
    total: Value,
    decimal: bool,
    binary: Option<Scratch>,
}
impl Default for Mean {
    fn default() -> Self {
        Self {
            total: Value::Int64(0),
            decimal: false,
            binary: None,
        }
    }
}
impl Mean {
    // The caller omits NULL, meters each record and checks cancellation.
    pub(super) fn push(
        &mut self,
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
        pool: &mut Vec<Scratch>,
    ) -> Result<(), QueryError> {
        match value {
            Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Currency(_) => {}
            Value::Decimal(_) => {
                self.decimal = true;
                if let Some(binary) = self.binary.take() {
                    self.total =
                        Value::Double(Double::from_float(binary.rounded(1, budget, cancelled)?));
                    binary.release(pool, budget)?;
                }
            }
            Value::Single(_) | Value::Double(_) if !self.decimal && self.binary.is_none() => {
                let mut binary = Scratch::acquire(pool, budget, cancelled)?;
                let Value::Double(prefix) = Conversion::Double.apply(&self.total)? else {
                    return Err(QueryError::Source);
                };
                binary.push(prefix.to_float())?;
                self.binary = Some(binary);
            }
            Value::Single(_) | Value::Double(_) => {}
            _ => return Err(EvalError::TypeMismatch.into()),
        }
        if let Some(binary) = &mut self.binary {
            let Value::Double(value) = Conversion::Double.apply(value)? else {
                return Err(QueryError::Source);
            };
            binary.push(value.to_float())?;
        } else {
            let total = Conversion::Decimal.apply(&self.total)?;
            let value = Conversion::Decimal.apply(value)?;
            self.total = numeric::arithmetic(Arithmetic::Add, &total, &value)?;
        }
        Ok(())
    }
    pub(super) fn finish(
        self,
        count: i64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
        pool: &mut Vec<Scratch>,
    ) -> Result<Value, QueryError> {
        if count == 0 {
            return Ok(Value::Null);
        }
        if let Some(binary) = self.binary {
            let result = binary.rounded(count, budget, cancelled)?;
            binary.release(pool, budget)?;
            return Ok(Value::Double(Double::from_float(result)));
        }
        let mean =
            numeric::promoted_arithmetic(Arithmetic::Divide, &self.total, &Value::Int64(count))?;
        Ok(if self.decimal {
            mean
        } else {
            Conversion::Double.apply(&mean)?
        })
    }
}
