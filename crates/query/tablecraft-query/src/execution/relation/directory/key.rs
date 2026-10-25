//! Exact fingerprints only after the whole join's same-carrier metadata proof.
use crate::execution::cell::CellValue;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_schema::ScalarType;

pub(super) fn valid(carrier: ScalarType, cell: Option<&CellValue>) -> bool {
    let Some(cell) = cell else {
        return false;
    };
    matches!(cell.value, ExpressionValue::Value(Value::Null))
        || matches!(
            (carrier, &cell.value),
            (
                ScalarType::Boolean,
                ExpressionValue::Value(Value::Boolean(_))
            ) | (ScalarType::Byte, ExpressionValue::Value(Value::Byte(_)))
                | (ScalarType::Int16, ExpressionValue::Value(Value::Int16(_)))
                | (ScalarType::Int32, ExpressionValue::Value(Value::Int32(_)))
                | (ScalarType::Int64, ExpressionValue::Value(Value::Int64(_)))
        )
}

pub(super) fn fingerprint(carrier: ScalarType, cell: Option<&CellValue>) -> Option<u64> {
    let mut hash = DefaultHasher::new();
    match (carrier, &cell?.value) {
        (ScalarType::Boolean, ExpressionValue::Value(Value::Boolean(value))) => {
            value.hash(&mut hash)
        }
        (ScalarType::Byte, ExpressionValue::Value(Value::Byte(value))) => value.hash(&mut hash),
        (ScalarType::Int16, ExpressionValue::Value(Value::Int16(value))) => value.hash(&mut hash),
        (ScalarType::Int32, ExpressionValue::Value(Value::Int32(value))) => value.hash(&mut hash),
        (ScalarType::Int64, ExpressionValue::Value(Value::Int64(value))) => value.hash(&mut hash),
        _ => return None,
    }
    Some(hash.finish())
}
