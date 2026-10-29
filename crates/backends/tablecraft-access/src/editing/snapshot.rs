//! Capture one complete owned scalar relation from its immutable native image.
use super::NativeEditError;
use tablecraft_ace::{InspectError, NativeImage, read_table};
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
    let name = native.schema().name();
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    let (_, columns, rows) = native.into_parts();
    Ok(Snapshot::new(
        owned,
        columns,
        rows,
        NATIVE_SCHEMA_POLICY,
        budget,
        cancelled,
    )?)
}
