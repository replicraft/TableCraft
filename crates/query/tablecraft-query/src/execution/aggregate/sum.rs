//! Exact floating totals; fixed-point and mixed-type admission stay explicit.
use super::binary::Scratch;
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

fn floating_operand(value: &Value) -> Result<Double, EvalError> {
    // Currency uses the existing checked decimal-to-binary conversion. Integral
    // operands still require exact representation; unsupported states stay errors.
    let converted = if matches!(value, Value::Currency(_)) {
        Conversion::Double.apply(value)?
    } else {
        let zero = Value::Double(Double::from_float(0.0));
        numeric::arithmetic(Arithmetic::Add, &zero, value)?
    };
    match converted {
        Value::Double(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}
