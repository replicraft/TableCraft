use super::Runtime;
use crate::profile::MAX_TEXT_BYTES;
use crate::{QueryError, source::TextPolicy};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::{comparison::Comparison, logic::Truth};

pub(super) fn scalar(value: &ExpressionValue) -> Result<&Value, QueryError> {
    match value {
        ExpressionValue::Value(value) => Ok(value),
        _ => Err(tablecraft_expressions::EvalError::NonScalarExpression.into()),
    }
}

impl Runtime<'_> {
    pub(super) fn copy_expression(
        &mut self,
        value: &ExpressionValue,
    ) -> Result<ExpressionValue, QueryError> {
        match value {
            ExpressionValue::Value(value) => self.copy(value).map(ExpressionValue::Value),
            _ => {
                self.tick()?;
                Ok(value.clone())
            }
        }
    }

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
    pub(super) fn copy_projected_row(
        &mut self,
        columns: &[tablecraft_schema::Column],
        projection: &[usize],
        row: crate::source::ProjectedRow<'_>,
    ) -> Result<Vec<ExpressionValue>, QueryError> {
        if row.len() != projection.len() {
            return Err(tablecraft_schema::SchemaError::Row.into());
        }
        let mut owned = self.allocate_expression_row(row.len())?;
        for (position, source_column) in projection.iter().copied().enumerate() {
            let column = columns.get(source_column).ok_or(QueryError::Source)?;
            let value = row.get(position).ok_or(QueryError::Source)?;
            tablecraft_schema::validate_row(
                std::slice::from_ref(column),
                std::slice::from_ref(value),
            )?;
            owned.push(ExpressionValue::Value(self.copy(value)?));
        }
        Ok(owned)
    }
    pub(super) fn equal(
        &mut self,
        left: &Value,
        right: &Value,
        policy: TextPolicy,
    ) -> Result<bool, QueryError> {
        self.tick()?;
        if matches!((left, right), (Value::Null, Value::Null)) {
            return Ok(true);
        }
        Ok(self.compare_policy(Comparison::Equal, left, right, policy)? == Truth::True)
    }
    pub(super) fn same_row(
        &mut self,
        left: &[Value],
        right: &[Value],
        policies: &[TextPolicy],
    ) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        if !policies.is_empty() && policies.len() != left.len() {
            return Err(QueryError::Width);
        }
        for (index, (l, r)) in left.iter().zip(right).enumerate() {
            if !self.equal(
                l,
                r,
                policies.get(index).copied().unwrap_or(TextPolicy::Default),
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn same_expression_row(
        &mut self,
        left: &[ExpressionValue],
        right: &[ExpressionValue],
        policies: &[TextPolicy],
    ) -> Result<bool, QueryError> {
        if left.len() != right.len() {
            return Ok(false);
        }
        if !policies.is_empty() && policies.len() != left.len() {
            return Err(QueryError::Width);
        }
        for (index, (left, right)) in left.iter().zip(right).enumerate() {
            if !self.equal(
                scalar(left)?,
                scalar(right)?,
                policies.get(index).copied().unwrap_or(TextPolicy::Default),
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(super) fn order(
        &mut self,
        left: &Value,
        right: &Value,
        policy: TextPolicy,
    ) -> Result<Ordering, QueryError> {
        self.tick()?;
        match (left, right) {
            (Value::Null, Value::Null) => return Ok(Ordering::Equal),
            (Value::Null, _) => return Ok(Ordering::Less),
            (_, Value::Null) => return Ok(Ordering::Greater),
            _ => {}
        }
        if self.compare_policy(Comparison::Equal, left, right, policy)? == Truth::True {
            return Ok(Ordering::Equal);
        }
        Ok(
            if self.compare_policy(Comparison::Less, left, right, policy)? == Truth::True {
                Ordering::Less
            } else {
                Ordering::Greater
            },
        )
    }
}
