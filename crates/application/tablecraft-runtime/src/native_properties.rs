//! Shared inert property preflight and exact native object binding.
use crate::{Error, controls::SCOPE};
use tablecraft_ace::{
    InspectError, NativeReadSource, TableProperties, TableSchema, inspect_table_properties,
    inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG: &str = "MSysObjects";
const PROPERTY_CARRIER: &str = "LvProp";

pub(crate) fn inspect<R: NativeReadSource>(
    source: &mut R,
    table: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<TableProperties>, Error> {
    let catalog = inspect_table_schema(source, SCOPE, CATALOG, work, cancelled)?;
    for column in catalog.columns() {
        crate::controls::check(cancelled)?;
        work.charge(
            Resource::Steps,
            1 + column.name().len() as u64 + PROPERTY_CARRIER.len() as u64,
        )?;
        if column.name().eq_ignore_ascii_case(PROPERTY_CARRIER) {
            return Ok(Some(inspect_table_properties(
                source, SCOPE, table, work, cancelled,
            )?));
        }
    }
    Ok(None)
}

pub(crate) fn require_binding(
    properties: Option<&TableProperties>,
    schema: &TableSchema,
) -> Result<(), Error> {
    if properties.is_some_and(|properties| properties.object_id() != schema.object_id()) {
        return Err(InspectError::Malformed.into());
    }
    Ok(())
}
