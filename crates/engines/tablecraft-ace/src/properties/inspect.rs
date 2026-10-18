use super::{TableProperties, decode};
use crate::{InspectError, inspect_catalog, table::read_selected};
use std::io::{Read, Seek};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::{CatalogScope, ObjectKind};

const SYSTEM_CATALOG: &str = "MSysObjects";
const OBJECT_ID_COLUMN: &str = "Id";
const PROPERTY_COLUMN: &str = "LvProp";
const LOCAL_TABLE_TYPE: i16 = 1;

/// Inspect inert property maps for one local table in the qualified read profile.
/// Unknown keys/types/flags and full LvProp bytes remain available. Rule/default
/// expressions are never executed. Missing properties remain absent; ordinary
/// schema/row APIs retain their existing conservative nullability policy.
pub fn inspect_table_properties<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableProperties, InspectError> {
    let _span = tracing::debug_span!("native.properties.read").entered();
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let object = catalog
        .catalog()
        .lookup(name, Some(ObjectKind::Table), None)?
        .ok_or(InspectError::UnknownTable)?;
    let origin = catalog
        .origins()
        .iter()
        .find(|origin| origin.object_id() == object.id())
        .ok_or(InspectError::Malformed)?;
    if origin.type_code() != LOCAL_TABLE_TYPE {
        return Err(InspectError::Unsupported);
    }
    let snapshot = read_selected(
        source,
        scope,
        SYSTEM_CATALOG,
        Some(&[OBJECT_ID_COLUMN, PROPERTY_COLUMN]),
        budget,
        cancelled,
    )?;
    let id_column = snapshot
        .columns()
        .iter()
        .position(|column| column.name().eq_ignore_ascii_case(OBJECT_ID_COLUMN))
        .ok_or(InspectError::Malformed)?;
    let properties_column = snapshot
        .columns()
        .iter()
        .position(|column| column.name().eq_ignore_ascii_case(PROPERTY_COLUMN))
        .ok_or(InspectError::Malformed)?;
    let mut rows = snapshot.into_rows();
    let mut matched = rows
        .iter_mut()
        .filter(|row| row.get(id_column) == Some(&Value::Int32(origin.native_id())));
    let row = matched.next().ok_or(InspectError::Malformed)?;
    let bytes = match std::mem::replace(
        row.get_mut(properties_column)
            .ok_or(InspectError::Malformed)?,
        Value::Null,
    ) {
        Value::Null => Vec::new(),
        Value::Binary(bytes) => bytes,
        _ => return Err(InspectError::Malformed),
    };
    if matched.next().is_some() {
        return Err(InspectError::Malformed);
    }
    let groups = decode::decode(&bytes, budget, cancelled)?;
    let name = object.name().as_str();
    budget.charge_allocation(name.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(TableProperties {
        name: owned,
        object: object.id(),
        raw: bytes,
        groups,
    })
}
