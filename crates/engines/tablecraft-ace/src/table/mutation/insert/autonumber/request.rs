//! Explicit generated-field omission and bounded copying of companion carriers.
use crate::{EditError, InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const COPY_CHUNK: usize = 1024;

/// Caller-selected incrementing engineering behavior and source precondition.
///
/// This does not detect or certify the source's stored New Values mode. There
/// is no default policy, random generation, explicit-ID override or reseeding.
#[derive(Clone, Copy)]
pub struct IncrementingAutoNumber {
    expected_last: i32,
}

impl IncrementingAutoNumber {
    /// Pin the last issued Int32 value in the original table-definition counter.
    pub fn new(expected_last: i32) -> Self {
        Self { expected_last }
    }

    /// Exact expected original counter, not the first generated value.
    pub fn expected_last(self) -> i32 {
        self.expected_last
    }
}

/// Explicit companion values in native column order, excluding the one
/// schema-resolved generated primary field. NULL remains a stored value.
pub struct AutoNumberRowInsert {
    values: Vec<Value>,
}

impl AutoNumberRowInsert {
    /// Capture companions; no value acts as a request to generate a field.
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// Companion values, excluding the generated column.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

/// Copy data only after its allocation and traversal are charged. Both producer
/// normalization and independent audit reconstruction use this carrier utility;
/// it does not select columns or derive generated values.
pub(in crate::table::mutation::insert) fn copy_value(
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    match value {
        Value::Text(text) => {
            budget
                .charge_allocation(text.len() as u64, size_of::<u8>() as u64)
                .map_err(InspectError::from)?;
            let mut copy = String::new();
            copy.try_reserve_exact(text.len())
                .map_err(|_| InspectError::Allocation)?;
            for character in text.chars() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, character.len_utf8() as u64)
                    .map_err(InspectError::from)?;
                copy.push(character);
            }
            Ok(Value::Text(copy))
        }
        Value::Binary(bytes) => {
            budget
                .charge_allocation(bytes.len() as u64, size_of::<u8>() as u64)
                .map_err(InspectError::from)?;
            let mut copy = Vec::new();
            copy.try_reserve_exact(bytes.len())
                .map_err(|_| InspectError::Allocation)?;
            for chunk in bytes.chunks(COPY_CHUNK) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, chunk.len() as u64)
                    .map_err(InspectError::from)?;
                copy.extend_from_slice(chunk);
            }
            Ok(Value::Binary(copy))
        }
        // Every remaining Value carrier is allocation-free. This match is
        // exhaustive so a future allocating carrier cannot inherit this path.
        Value::Null
        | Value::Boolean(_)
        | Value::Byte(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::Single(_)
        | Value::Double(_)
        | Value::Currency(_)
        | Value::Decimal(_)
        | Value::Guid(_)
        | Value::DateTime(_)
        | Value::DateTimeExtended(_) => Ok(value.clone()),
    }
}
