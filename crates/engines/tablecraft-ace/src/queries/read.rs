use super::{SavedQueryDefinition, SavedQueryRow, columns, record};
use crate::{InspectError, header::check, inspect_catalog, read_table};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::{CatalogScope, ObjectKind};

const SYSTEM_TABLE: &str = "MSysQueries";

/// Read one explicitly named saved query's stored native fragments on a stable/
/// offline source. Preserve signed IDs, NULL/unknown fields and storage order.
/// No SQL reconstruction/validation, provider access or active content executes.
pub fn read_saved_query_definition<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SavedQueryDefinition, InspectError> {
    let _span = tracing::debug_span!("native.query.definition").entered();
    check(budget, cancelled)?;
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    budget.charge(
        Resource::Steps,
        catalog.catalog().objects().len() as u64 * 2,
    )?;
    let object = catalog
        .catalog()
        .lookup(name, Some(ObjectKind::Query), None)?
        .ok_or(InspectError::UnknownObject)?;
    let position = catalog
        .catalog()
        .objects()
        .iter()
        .position(|candidate| candidate.id() == object.id())
        .ok_or(InspectError::Malformed)?;
    let native_id = catalog.origins()[position].native_id();
    let table = read_table(source, scope, SYSTEM_TABLE, budget, cancelled)?;
    let columns = columns::read(&table, budget, cancelled)?;
    let mut count = 0;
    for row in table.rows() {
        check(budget, cancelled)?;
        if row.get(columns.positions[columns::OBJECT_ID]) == Some(&Value::Int32(native_id)) {
            count += 1;
        }
    }
    budget.charge_allocation(count as u64, size_of::<SavedQueryRow>() as u64)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for row in table.into_rows() {
        check(budget, cancelled)?;
        if row.get(columns.positions[columns::OBJECT_ID]) == Some(&Value::Int32(native_id)) {
            budget.charge(Resource::Steps, columns::FIELD_COUNT as u64)?;
            rows.push(record::read(row, &columns, native_id)?);
        }
    }
    check(budget, cancelled)?;
    Ok(SavedQueryDefinition {
        catalog,
        position,
        rows,
    })
}
