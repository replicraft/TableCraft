use super::{
    QueryAttribute, SavedQueryRow,
    columns::{self, Columns},
};
use crate::InspectError;
use tablecraft_core::values::Value;

pub(super) fn read(
    row: Vec<Value>,
    columns: &Columns,
    object_id: i32,
) -> Result<SavedQueryRow, InspectError> {
    let mut cells: [Value; columns::FIELD_COUNT] =
        row.try_into().map_err(|_| InspectError::Malformed)?;
    let mut take = |field| std::mem::replace(&mut cells[columns.positions[field]], Value::Null);
    if take(columns::OBJECT_ID) != Value::Int32(object_id) {
        return Err(InspectError::Malformed);
    }
    let attribute = match take(columns::ATTRIBUTE) {
        Value::Null => None,
        Value::Byte(value) => Some(QueryAttribute::from_code(value)),
        _ => return Err(InspectError::Malformed),
    };
    let flag = match take(columns::FLAG) {
        Value::Null => None,
        Value::Int16(value) => Some(value),
        _ => return Err(InspectError::Malformed),
    };
    let extra = match take(columns::EXTRA) {
        Value::Null => None,
        Value::Int32(value) => Some(value),
        _ => return Err(InspectError::Malformed),
    };
    let order = match take(columns::ORDER) {
        Value::Null => None,
        Value::Binary(value) => Some(value),
        _ => return Err(InspectError::Malformed),
    };
    Ok(SavedQueryRow {
        object_id,
        attribute,
        flag,
        extra,
        order,
        expression: text(take(columns::EXPRESSION))?,
        name1: text(take(columns::NAME1))?,
        name2: text(take(columns::NAME2))?,
    })
}
fn text(value: Value) -> Result<Option<String>, InspectError> {
    match value {
        Value::Null => Ok(None),
        Value::Text(value) => Ok(Some(value)),
        _ => Err(InspectError::Malformed),
    }
}
