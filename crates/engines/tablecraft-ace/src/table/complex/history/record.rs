use super::{StoredVersion, schema::Columns};
use crate::InspectError;
use tablecraft_core::values::Value;

pub(super) fn read<'a>(
    row: &'a [Value],
    columns: &Columns,
) -> Result<StoredVersion<'a>, InspectError> {
    let identity = match row.get(columns.identity) {
        Some(Value::Int32(id)) if *id > 0 => *id,
        _ => return Err(InspectError::Malformed),
    };
    let value = match row.get(columns.value) {
        Some(Value::Null) => None,
        Some(Value::Text(value)) => Some(value.as_str()),
        _ => return Err(InspectError::Malformed),
    };
    let modified = match row.get(columns.modified) {
        Some(Value::Null) => None,
        Some(Value::DateTime(value)) => Some(*value),
        _ => return Err(InspectError::Malformed),
    };
    Ok(StoredVersion {
        identity,
        value,
        modified,
    })
}
