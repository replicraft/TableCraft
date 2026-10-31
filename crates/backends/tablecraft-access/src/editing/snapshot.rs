//! Capture one complete owned scalar relation from its immutable native image.
use super::NativeEditError;
use tablecraft_ace::{InspectError, NativeImage, TableSchema, TableSnapshot, read_table};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
use tablecraft_ace::{inspect_table_schema, read_table_projection};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
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

#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(crate) struct ProjectedSnapshot {
    pub snapshot: Snapshot,
    #[cfg(feature = "ace16-write-preview")]
    pub schema: TableSchema,
    #[cfg(feature = "ace16-write-preview")]
    pub projected: bool,
}

/// Actual scalar projection. Excluded declarations do not become NULL cells.
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(crate) fn capture_projected(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProjectedSnapshot, NativeEditError> {
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
        let (snapshot, _schema) = finish(native, budget, cancelled)?;
        return Ok(ProjectedSnapshot {
            snapshot,
            #[cfg(feature = "ace16-write-preview")]
            schema: _schema,
            #[cfg(feature = "ace16-write-preview")]
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
    let (snapshot, _schema) = finish(native, budget, cancelled)?;
    Ok(ProjectedSnapshot {
        snapshot,
        #[cfg(feature = "ace16-write-preview")]
        schema: _schema,
        #[cfg(feature = "ace16-write-preview")]
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

#[cfg(feature = "ace16-write-preview")]
pub(crate) fn capture_engineering(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProjectedSnapshot, NativeEditError> {
    capture_projected(image, table, budget, cancelled)
}
