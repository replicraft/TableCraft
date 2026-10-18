use super::Runtime;
use crate::QueryError;
use crate::profile::MAX_TEXT_BYTES;
use std::cmp::Ordering;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_expressions::{
    comparison::{self, Comparison},
    logic::Truth,
};

impl Runtime<'_> {
    pub(super) fn copy(&mut self, value: &Value) -> Result<Value, QueryError> {
        self.tick()?;
        Ok(match value {
            Value::Text(text) => {
                if text.len() > MAX_TEXT_BYTES {
                    return Err(tablecraft_expressions::EvalError::TextLimit.into());
                }
                self.budget.charge(Resource::Bytes, text.len() as u64)?;
                let mut owned = String::new();
                owned
                    .try_reserve_exact(text.len())
                    .map_err(|_| QueryError::Allocation)?;
                owned.push_str(text);
                Value::Text(owned)
            }
            Value::Binary(bytes) => {
                self.budget.charge(Resource::Bytes, bytes.len() as u64)?;
                let mut owned = Vec::new();
                owned
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| QueryError::Allocation)?;
                owned.extend_from_slice(bytes);
                Value::Binary(owned)
            }
            Value::Single(v) if !v.to_float().is_finite() => {
                return Err(tablecraft_expressions::EvalError::NonFinite.into());
            }
            Value::Double(v) if !v.to_float().is_finite() => {
                return Err(tablecraft_expressions::EvalError::NonFinite.into());
            }
            _ => value.clone(),
        })
    }
    pub(super) fn copy_row(&mut self, row: &[Value]) -> Result<Vec<Value>, QueryError> {
        let mut owned = self.allocate_row(row.len())?;
        for v in row {
            owned.push(self.copy(v)?);
        }
        Ok(owned)
    }
    pub(super) fn equal(&mut self, left: &Value, right: &Value) -> Result<bool, QueryError> {
        self.tick()?;
        if matches!((left, right), (Value::Null, Value::Null)) {
            return Ok(true);
        }
        Ok(
            comparison::compare(Comparison::Equal, left, right, self.options.collation)?
                == Truth::True,
        )
    }
    pub(super) fn same_row(&mut self, left: &[Value], right: &[Value]) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (l, r) in left.iter().zip(right) {
            if !self.equal(l, r)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(super) fn order(&mut self, left: &Value, right: &Value) -> Result<Ordering, QueryError> {
        self.tick()?;
        match (left, right) {
            (Value::Null, Value::Null) => return Ok(Ordering::Equal),
            (Value::Null, _) => return Ok(Ordering::Less),
            (_, Value::Null) => return Ok(Ordering::Greater),
            _ => {}
        }
        if comparison::compare(Comparison::Equal, left, right, self.options.collation)?
            == Truth::True
        {
            return Ok(Ordering::Equal);
        }
        Ok(
            if comparison::compare(Comparison::Less, left, right, self.options.collation)?
                == Truth::True
            {
                Ordering::Less
            } else {
                Ordering::Greater
            },
        )
    }
}
