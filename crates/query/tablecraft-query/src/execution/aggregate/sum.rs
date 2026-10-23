//! Exact floating totals; fixed-point and mixed-type admission stay explicit.
use super::binary::Scratch;
use crate::QueryError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};
use tablecraft_expressions::numeric::{self, Arithmetic};

pub(super) struct Sum {
    total: Value,
    binary: Option<Scratch>,
    seen: bool,
}

impl Default for Sum {
    fn default() -> Self {
        Self {
            total: Value::Int64(0),
            binary: None,
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
        if matches!(value, Value::Decimal(_))
            && let Some(binary) = self.binary.take()
        {
            self.total = Value::Double(Double::from_float(binary.rounded(1, budget, cancelled)?));
            binary.release(pool, budget)?;
        }
        let floating = matches!(value, Value::Single(_) | Value::Double(_));
        if (floating || self.binary.is_some())
            && !matches!(self.total, Value::Currency(_) | Value::Decimal(_))
        {
            let zero = Value::Double(Double::from_float(0.0));
            // Reuse strict admission/precision checks, including exact integral
            // conversion and Currency refusal. No unsupported value becomes zero.
            let Value::Double(value) = numeric::arithmetic(Arithmetic::Add, &zero, value)? else {
                return Err(QueryError::Source);
            };
            if self.binary.is_none() {
                let Value::Double(prefix) =
                    numeric::arithmetic(Arithmetic::Add, &zero, &self.total)?
                else {
                    return Err(QueryError::Source);
                };
                let mut binary = Scratch::acquire(pool, budget, cancelled)?;
                binary.push(prefix.to_float())?;
                self.binary = Some(binary);
            }
            self.binary
                .as_mut()
                .ok_or(QueryError::Source)?
                .push(value.to_float())?;
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
        } else if self.seen {
            Ok(self.total)
        } else {
            Ok(Value::Null)
        }
    }
}
