//! Exact Text/integral heading names; locale-dependent formatting is not guessed.
use super::*;
use std::{cmp::Ordering, fmt::Write};
use tablecraft_core::limits::Resource;

// Decimal digits plus a sign; i64::MIN fits the same width.
const INTEGER_NAME_BYTES: usize = i64::MAX.ilog10() as usize + 1 + 1;

impl Runtime<'_> {
    pub(super) fn add_heading(
        &mut self,
        headings: &mut Vec<Value>,
        value: &Value,
        width: usize,
        fixed: bool,
    ) -> Result<(), QueryError> {
        self.tick()?;
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
            _ => return Err(QueryError::Unsupported),
        }
        for existing in headings.iter() {
            if self.equal(existing, value)? {
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
        let owned = self.copy(value)?;
        self.push(headings, owned)
    }
    pub(super) fn sort_headings(
        &mut self,
        mut headings: Vec<Value>,
    ) -> Result<Vec<Value>, QueryError> {
        // Width is capped at 256. Fallible insertion ordering checks work and
        // cancellation on every comparison and never clones key payloads.
        for start in 1..headings.len() {
            let mut index = start;
            while index > 0 && self.order(&headings[index], &headings[index - 1])? == Ordering::Less
            {
                headings.swap(index, index - 1);
                index -= 1;
            }
        }
        Ok(headings)
    }
    pub(super) fn heading_name(&mut self, value: &Value) -> Result<String, QueryError> {
        match value {
            Value::Text(text) => self.copy_heading_name(text),
            Value::Byte(value) => self.integer_heading(i64::from(*value)),
            Value::Int16(value) => self.integer_heading(i64::from(*value)),
            Value::Int32(value) => self.integer_heading(i64::from(*value)),
            Value::Int64(value) => self.integer_heading(*value),
            _ => Err(QueryError::Unsupported),
        }
    }
    fn integer_heading(&mut self, value: i64) -> Result<String, QueryError> {
        self.tick()?;
        self.budget
            .charge(Resource::Bytes, INTEGER_NAME_BYTES as u64)?;
        let mut name = String::new();
        name.try_reserve_exact(INTEGER_NAME_BYTES)
            .map_err(|_| QueryError::Allocation)?;
        write!(&mut name, "{value}").map_err(|_| QueryError::Allocation)?;
        Ok(name)
    }
    fn validate_heading_name(&mut self, text: &str) -> Result<(), QueryError> {
        self.tick()?;
        if text.is_empty()
            || text.len() > SOURCE_SCHEMA_POLICY.max_name_bytes
            || text.chars().any(char::is_control)
        {
            return Err(QueryError::Unsupported);
        }
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
