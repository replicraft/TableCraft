//! Exact native scalar payload bytes, shared by mutation and fresh creation.
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

pub(crate) fn encode<const WIDTH: usize>(kind: ScalarType, value: &Value) -> Option<[u8; WIDTH]> {
    match (kind, value) {
        (ScalarType::Byte, Value::Byte(value)) => bytes([*value]),
        (ScalarType::Int16, Value::Int16(value)) => bytes(value.to_le_bytes()),
        (ScalarType::Int32, Value::Int32(value)) => bytes(value.to_le_bytes()),
        (ScalarType::Int64, Value::Int64(value)) => bytes(value.to_le_bytes()),
        (ScalarType::Currency, Value::Currency(value)) => bytes(value.scaled().to_le_bytes()),
        (ScalarType::Single, Value::Single(value)) => bytes(value.to_bits().to_le_bytes()),
        (ScalarType::Double, Value::Double(value)) => bytes(value.to_bits().to_le_bytes()),
        (ScalarType::DateTime, Value::DateTime(value)) => bytes(value.to_bits().to_le_bytes()),
        (ScalarType::Guid, Value::Guid(value)) => {
            bytes(uuid::Uuid::from_bytes(value.to_bytes()).to_bytes_le())
        }
        _ => None,
    }
}

fn bytes<const WIDTH: usize, const SOURCE: usize>(value: [u8; SOURCE]) -> Option<[u8; WIDTH]> {
    value.as_slice().try_into().ok()
}
