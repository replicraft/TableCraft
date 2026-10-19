use super::{
    metadata::StoredAttachment,
    schema::{self, Columns},
};
use crate::InspectError;
use tablecraft_core::values::Value;

pub(super) fn read<'a>(
    row: &'a [Value],
    columns: &Columns,
) -> Result<StoredAttachment<'a>, InspectError> {
    let field = |position| {
        row.get(columns.positions[position])
            .ok_or(InspectError::Malformed)
    };
    let identity = match row.get(columns.identity) {
        Some(Value::Int32(identity)) if *identity > 0 => *identity,
        _ => return Err(InspectError::Malformed),
    };
    let encoded = match field(schema::DATA)? {
        Value::Null => None,
        Value::Binary(bytes) => Some(bytes.as_slice()),
        _ => return Err(InspectError::Malformed),
    };
    let timestamp = match field(schema::TIMESTAMP)? {
        Value::Null => None,
        Value::DateTime(value) => Some(*value),
        _ => return Err(InspectError::Malformed),
    };
    let flags = match field(schema::FLAGS)? {
        Value::Null => None,
        Value::Int32(value) => Some(*value),
        _ => return Err(InspectError::Malformed),
    };
    Ok(StoredAttachment {
        identity,
        url: text(field(schema::URL)?)?,
        name: text(field(schema::NAME)?)?,
        file_type: text(field(schema::TYPE)?)?,
        encoded,
        timestamp,
        flags,
    })
}
fn text(value: &Value) -> Result<Option<&str>, InspectError> {
    match value {
        Value::Null => Ok(None),
        Value::Text(text) => Ok(Some(text)),
        _ => Err(InspectError::Malformed),
    }
}
