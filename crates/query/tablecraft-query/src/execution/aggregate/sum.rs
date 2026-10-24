//! Exact floating and wider Decimal totals; other prefix admission stays explicit.
use super::{binary::Scratch, fixed_total};
use crate::QueryError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_expressions::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{self, Arithmetic},
};

pub(super) struct Sum {
    total: Value,
    binary: Option<Scratch>,
    decimal: Option<fixed_total::Total>,
    seen: bool,
}

impl Default for Sum {
    fn default() -> Self {
        Self {
            total: Value::Int64(0),
            binary: None,
            decimal: None,
            seen: false,
        }
    }
}

impl Sum {
    // The caller evaluates each operand once, omits NULL and meters every row.
    pub(super) fn push(
        &mut self,
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
        pool: &mut Vec<Scratch>,
    ) -> Result<(), QueryError> {
        if let Some(decimal) = &mut self.decimal {
            let Value::Decimal(value) = Conversion::Decimal.apply(value)? else {
                return Err(QueryError::Source);
            };
            decimal.push(value, budget, cancelled)?;
            self.seen = true;
            return Ok(());
        }
        if matches!(value, Value::Decimal(_))
            && let Some(binary) = self.binary.take()
        {
            self.total = Value::Double(Double::from_float(binary.rounded(1, budget, cancelled)?));
            binary.release(pool, budget)?;
        }
        let floating = matches!(value, Value::Single(_) | Value::Double(_));
        if (floating || self.binary.is_some()) && !matches!(self.total, Value::Decimal(_)) {
            let value = floating_operand(value)?;
            if self.binary.is_none() {
                let prefix = floating_operand(&self.total)?;
                let mut binary = Scratch::acquire(pool, budget, cancelled)?;
                binary.push(prefix.to_float())?;
                self.binary = Some(binary);
            }
            self.binary
                .as_mut()
                .ok_or(QueryError::Source)?
                .push(value.to_float())?;
        } else if matches!(self.total, Value::Decimal(_)) || matches!(value, Value::Decimal(_)) {
            let Value::Decimal(left) = Conversion::Decimal.apply(&self.total)? else {
                return Err(QueryError::Source);
            };
            let Value::Decimal(right) = Conversion::Decimal.apply(value)? else {
                return Err(QueryError::Source);
            };
            if fixed_total::narrow_sum(left, right) {
                self.total = numeric::arithmetic(
                    Arithmetic::Add,
                    &Value::Decimal(left),
                    &Value::Decimal(right),
                )?;
            } else {
                let mut total = fixed_total::Total::new(left);
                total.push(right, budget, cancelled)?;
                self.decimal = Some(total);
            }
        } else {
            self.total = numeric::arithmetic(Arithmetic::Add, &self.total, value)?;
        }
        self.seen = true;
        Ok(())
    }

    pub(super) fn finish(
        self,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
        pool: &mut Vec<Scratch>,
    ) -> Result<Value, QueryError> {
        if let Some(binary) = self.binary {
            let total = binary.rounded(1, budget, cancelled)?;
            binary.release(pool, budget)?;
            Ok(Value::Double(Double::from_float(total)))
        } else if let Some(decimal) = self.decimal {
            decimal.sum(budget, cancelled)
        } else if self.seen {
            Ok(self.total)
        } else {
            Ok(Value::Null)
        }
    }
}

fn floating_operand(value: &Value) -> Result<Double, EvalError> {
    // Floating-selected sums round numeric operands through the same converter
    // as means. Keep DATE/Text admission separate from explicit CDbl calls.
    if !matches!(
        value,
        Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Currency(_)
            | Value::Single(_)
            | Value::Double(_)
    ) {
        return Err(EvalError::TypeMismatch);
    }

    let converted = Conversion::Double.apply(value)?;
    match converted {
        Value::Double(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}
