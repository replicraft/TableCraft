//! Typed pivot keys with invariant Text/integer or explicit-context scalar names.
use super::*;
use crate::heading_names;
use std::cmp::Ordering;
use tablecraft_core::limits::Resource;
use tablecraft_expressions::functions::text::TextFunction;

impl Runtime<'_, '_> {
    pub(super) fn add_heading(
        &mut self,
        headings: &mut Vec<CellValue>,
        cell: &CellValue,
        width: usize,
        fixed: bool,
        policy: crate::source::TextPolicy,
    ) -> Result<(), QueryError> {
        self.tick()?;
        let value = super::super::values::scalar(cell)?;
        if matches!(value, Value::Null) {
            if fixed {
                return Err(QueryError::Unsupported);
            }
            return Ok(());
        }
        // Validate the name even if collation would merge this representative.
        match value {
            Value::Text(text) => self.validate_heading_name(text)?,
            Value::Byte(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_) => {}
            Value::Boolean(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_)
            | Value::DateTimeExtended(_) => {
                // A duplicate cannot hide missing policy or an invalid name.
                self.formatted_heading(value)?;
            }
            _ => return Err(QueryError::Unsupported),
        }
        for existing in headings.iter_mut() {
            if self.equal(
                super::super::values::scalar(existing)?,
                value,
                policy.merge(existing.policy).merge(cell.policy),
            )? {
                existing.policy = existing.policy.merge(cell.policy);
                return if fixed {
                    Err(QueryError::AmbiguousName)
                } else {
                    Ok(())
                };
            }
        }
        if headings.len() + width >= SOURCE_SCHEMA_POLICY.max_columns {
            return Err(QueryError::StructureLimit);
        }
        let owned = self.copy_expression(cell)?;
        self.push(headings, owned)
    }
    pub(super) fn sort_headings(
        &mut self,
        mut headings: Vec<CellValue>,
        policy: crate::source::TextPolicy,
    ) -> Result<Vec<CellValue>, QueryError> {
        // Width is capped at 256. Fallible insertion ordering checks work and
        // cancellation on every comparison and never clones key payloads.
        for start in 1..headings.len() {
            let mut index = start;
            while index > 0
                && self.order(
                    super::super::values::scalar(&headings[index])?,
                    super::super::values::scalar(&headings[index - 1])?,
                    policy
                        .merge(headings[index].policy)
                        .merge(headings[index - 1].policy),
                )? == Ordering::Less
            {
                headings.swap(index, index - 1);
                index -= 1;
            }
        }
        Ok(headings)
    }
    pub(super) fn check_fixed_heading_policy(
        &mut self,
        headings: &[CellValue],
        policy: crate::source::TextPolicy,
    ) -> Result<(), QueryError> {
        for (position, heading) in headings.iter().enumerate() {
            for previous in &headings[..position] {
                if self.equal(
                    super::super::values::scalar(heading)?,
                    super::super::values::scalar(previous)?,
                    policy.merge(heading.policy).merge(previous.policy),
                )? {
                    return Err(QueryError::AmbiguousName);
                }
            }
        }
        Ok(())
    }
    pub(super) fn heading_name(&mut self, value: &Value) -> Result<String, QueryError> {
        match value {
            Value::Text(text) => self.copy_heading_name(text),
            Value::Byte(value) => self.integer_heading(i64::from(*value)),
            Value::Int16(value) => self.integer_heading(i64::from(*value)),
            Value::Int32(value) => self.integer_heading(i64::from(*value)),
            Value::Int64(value) => self.integer_heading(*value),
            Value::Boolean(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_)
            | Value::DateTimeExtended(_) => self.formatted_heading(value),
            _ => Err(QueryError::Unsupported),
        }
    }
    fn formatted_heading(&mut self, value: &Value) -> Result<String, QueryError> {
        let Value::Text(text) = self
            .evaluator()
            .apply_text_function(TextFunction::CStr, std::slice::from_ref(value))?
        else {
            return Err(QueryError::Unsupported);
        };
        self.validate_heading_name(&text)?;
        Ok(text)
    }
    fn integer_heading(&mut self, value: i64) -> Result<String, QueryError> {
        self.tick()?;
        self.budget
            .charge(Resource::Bytes, heading_names::INTEGER_NAME_BYTES as u64)?;
        heading_names::integer(value)
    }
    fn validate_heading_name(&mut self, text: &str) -> Result<(), QueryError> {
        self.tick()?;
        heading_names::validate(text)?;
        self.budget.charge(Resource::Steps, text.len() as u64)?;
        Ok(())
    }
    pub(super) fn copy_heading_name(&mut self, text: &str) -> Result<String, QueryError> {
        self.validate_heading_name(text)?;
        self.budget.charge(Resource::Bytes, text.len() as u64)?;
        let mut name = String::new();
        name.try_reserve_exact(text.len())
            .map_err(|_| QueryError::Allocation)?;
        name.push_str(text);
        Ok(name)
    }
}
