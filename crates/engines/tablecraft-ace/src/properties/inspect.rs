use super::{TableProperties, decode};
use crate::{InspectError, inspect_catalog};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};

mod raw;
pub(super) use raw::read_property_bytes;

const LOCAL_TABLE_TYPE: i16 = 1;

/// Inspect inert property maps for one local table in the qualified read profile.
/// Unknown keys/types/flags and full LvProp bytes remain available. Rule/default
/// expressions are never executed. Missing properties remain absent; ordinary
/// schema/row APIs retain their existing conservative nullability policy.
pub fn inspect_table_properties<R: crate::NativeReadSource>(
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
    let bytes = read_property_bytes(source, scope, origin.native_id(), budget, cancelled)?;
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
