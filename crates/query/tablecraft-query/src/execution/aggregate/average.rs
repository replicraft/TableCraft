//! Wider exact totals for means; source carriers and SUM retain their own rules.
mod binary;
mod wide;
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
    wide: Option<wide::Total>,
    binary: Option<Scratch>,
}
impl Default for Mean {
    fn default() -> Self {
        Self {
            total: Value::Int64(0),
            decimal: false,
            wide: None,
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
                let prefix = if let Some(wide) = self.wide.take() {
                    wide.double(1, budget, cancelled)?
                } else {
                    let Value::Double(prefix) = Conversion::Double.apply(&self.total)? else {
                        return Err(QueryError::Source);
                    };
                    prefix.to_float()
                };
                binary.push(prefix)?;
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
            let Value::Decimal(value) = Conversion::Decimal.apply(value)? else {
                return Err(QueryError::Source);
            };
            if let Some(wide) = &mut self.wide {
                wide.push(value, budget, cancelled)?;
            } else {
                let Value::Decimal(total) = Conversion::Decimal.apply(&self.total)? else {
                    return Err(QueryError::Source);
                };
                if wide::narrow_sum(total, value) {
                    self.total = numeric::arithmetic(
                        Arithmetic::Add,
                        &Value::Decimal(total),
                        &Value::Decimal(value),
                    )?;
                } else {
                    let mut total = wide::Total::new(total);
                    total.push(value, budget, cancelled)?;
                    self.wide = Some(total);
                }
            }
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
        if let Some(wide) = self.wide {
            return if self.decimal {
                wide.decimal(count, budget, cancelled)
            } else {
                Ok(Value::Double(Double::from_float(
                    wide.double(count, budget, cancelled)?,
                )))
            };
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
