//! Capture one complete owned scalar relation from its immutable native image.
use super::NativeEditError;
use tablecraft_ace::{InspectError, NativeImage, TableSchema, TableSnapshot, read_table};
#[cfg(feature = "ace16-write-preview")]
use tablecraft_ace::{inspect_table_schema, read_table_projection};
#[cfg(feature = "ace16-write-preview")]
use tablecraft_core::limits::Resource;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::Snapshot;
use tablecraft_schema::{NameComparison, SchemaPolicy};

const NATIVE_SCHEMA_POLICY: SchemaPolicy = SchemaPolicy {
    max_name_bytes: 255,
    max_columns: 256,
    names: NameComparison::AsciiInsensitive,
};

pub(crate) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Snapshot, NativeEditError> {
    let native = read_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    finish(native, budget, cancelled).map(|(snapshot, _)| snapshot)
}

#[cfg(feature = "ace16-write-preview")]
pub(crate) struct EngineeringSnapshot {
    pub snapshot: Snapshot,
    pub schema: TableSchema,
    pub projected: bool,
}

/// Engineering-only ordinary projection. Excluded declarations remain available
/// to the editor; no cached calculation or collection becomes a scalar NULL.
#[cfg(feature = "ace16-write-preview")]
pub(crate) fn capture_engineering(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringSnapshot, NativeEditError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let mut projected = false;
    for field in schema.columns() {
        if cancelled() {
            return Err(InspectError::Cancelled.into());
        }
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        projected |= field.scalar_type().is_none();
    }
    if !projected {
        let native = read_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        let (snapshot, schema) = finish(native, budget, cancelled)?;
        return Ok(EngineeringSnapshot {
            snapshot,
            schema,
            projected: false,
        });
    }
    let metadata = crate::source::schema::classify(&schema, budget, cancelled)?;
    let names = metadata.projection_names(budget)?;
    let native = read_table_projection(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        &names,
        budget,
        cancelled,
    )?;
    if !metadata.matches(&native) {
        return Err(InspectError::Malformed.into());
    }
    let (snapshot, schema) = finish(native, budget, cancelled)?;
    Ok(EngineeringSnapshot {
        snapshot,
        schema,
        projected: true,
    })
}

fn finish(
    native: TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Snapshot, TableSchema), NativeEditError> {
    let name = native.schema().name();
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    let (schema, columns, rows) = native.into_parts();
    let snapshot = Snapshot::new(
        owned,
        columns,
        rows,
        NATIVE_SCHEMA_POLICY,
        budget,
        cancelled,
    )?;
    Ok((snapshot, schema))
}
