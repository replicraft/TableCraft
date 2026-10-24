use super::Runtime;
use crate::execution::cell::CellValue;
use crate::{QueryError, source::TextPolicy};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionString, ExpressionValue, Value},
};
use tablecraft_expressions::{comparison::Comparison, logic::Truth, numeric::OperandDeclaration};

const STRING_COPY_CHUNK_BYTES: usize = 4096;

pub(super) fn scalar(value: &CellValue) -> Result<&Value, QueryError> {
    match &value.value {
        ExpressionValue::Value(value) => Ok(value),
        _ => Err(tablecraft_expressions::EvalError::NonScalarExpression.into()),
    }
}

impl Runtime<'_, '_> {
    pub(super) fn materialize_strings(
        &mut self,
        cells: &mut [CellValue],
    ) -> Result<(), QueryError> {
        for cell in cells {
            self.tick()?;
            if let ExpressionValue::StringBytes(bytes) = &cell.value {
                cell.value =
                    ExpressionValue::Value(self.evaluator().publish_expression_string(bytes)?);
            }
        }
        Ok(())
    }

    pub(super) fn copy_expression(&mut self, value: &CellValue) -> Result<CellValue, QueryError> {
        let copied = match &value.value {
            ExpressionValue::Value(value) => ExpressionValue::Value(self.copy(value)?),
            ExpressionValue::StringBytes(bytes) => {
                self.tick()?;
                self.budget.charge(Resource::Bytes, bytes.len() as u64)?;
                let mut owned = Vec::new();
                owned
                    .try_reserve_exact(bytes.len())
                    .map_err(|_| QueryError::Allocation)?;
                for chunk in bytes.as_bytes().chunks(STRING_COPY_CHUNK_BYTES) {
                    self.tick()?;
                    owned.extend_from_slice(chunk);
                }
                ExpressionValue::StringBytes(ExpressionString::from_owned(owned))
            }
            value => {
                self.tick()?;
                value.clone()
            }
        };
        Ok(CellValue::from(copied)
            .with_policy(value.policy)
            .with_declaration(value.declaration))
    }

    pub(super) fn unify_cell_policies<T>(
        &mut self,
        rows: &mut [T],
        cells: fn(&mut T) -> &mut [CellValue],
    ) -> Result<(), QueryError> {
        if self.options.collation_resolver.is_none() {
            return Ok(());
        }
        let width = rows.first_mut().map_or(0, |row| cells(row).len());
        for column in 0..width {
            let mut policy = TextPolicy::Default;
            for row in rows.iter_mut() {
                self.tick()?;
                policy = policy.merge(cells(row).get(column).ok_or(QueryError::Width)?.policy);
            }
            for row in rows.iter_mut() {
                self.tick()?;
                cells(row).get_mut(column).ok_or(QueryError::Width)?.policy = policy;
            }
        }
        Ok(())
    }

    pub(super) fn copy(&mut self, value: &Value) -> Result<Value, QueryError> {
        self.tick()?;
        Ok(match value {
            Value::Text(text) => {
                if text.len() > self.options.text_limit() {
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
    ) -> Result<Vec<CellValue>, QueryError> {
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
            owned.push(
                CellValue::scalar(self.copy(value)?)
                    .with_declaration(Some(OperandDeclaration::Scalar)),
            );
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
    pub(super) fn same_expression_row(
        &mut self,
        left: &[CellValue],
        right: &[CellValue],
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
                policies
                    .get(index)
                    .copied()
                    .unwrap_or(TextPolicy::Default)
                    .merge(left.policy)
                    .merge(right.policy),
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
