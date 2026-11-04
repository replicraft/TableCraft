//! Exact catalog identity-to-LvProp association shared by inert property readers.
use crate::{InspectError, NativeReadSource, header::check, table::read_selected};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SYSTEM_CATALOG: &str = "MSysObjects";
const OBJECT_ID_COLUMN: &str = "Id";
const PROPERTY_COLUMN: &str = "LvProp";

pub(in crate::properties) fn read_property_bytes<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    native_id: i32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, InspectError> {
    let snapshot = read_selected(
        source,
        scope,
        SYSTEM_CATALOG,
        Some(&[OBJECT_ID_COLUMN, PROPERTY_COLUMN]),
        budget,
        cancelled,
    )?;
    let mut id_column = None;
    let mut properties_column = None;
    for (position, column) in snapshot.columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (2 * column.name().len() + OBJECT_ID_COLUMN.len() + PROPERTY_COLUMN.len()) as u64,
        )?;
        if column.name().eq_ignore_ascii_case(OBJECT_ID_COLUMN) {
            id_column = Some(position);
        }
        if column.name().eq_ignore_ascii_case(PROPERTY_COLUMN) {
            properties_column = Some(position);
        }
    }
    let id_column = id_column.ok_or(InspectError::Malformed)?;
    let properties_column = properties_column.ok_or(InspectError::Malformed)?;
    let mut matched = None;
    for mut row in snapshot.into_rows() {
        check(budget, cancelled)?;
        if row.get(id_column) != Some(&Value::Int32(native_id)) {
            continue;
        }
        if matched.is_some() {
            return Err(InspectError::Malformed);
        }
        let property = row
            .get_mut(properties_column)
            .ok_or(InspectError::Malformed)?;
        matched = Some(match std::mem::replace(property, Value::Null) {
            Value::Null => Vec::new(),
            Value::Binary(bytes) => bytes,
            _ => return Err(InspectError::Malformed),
        });
    }
    check(budget, cancelled)?;
    matched.ok_or(InspectError::Malformed)
}
