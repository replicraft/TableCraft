//! Explicit scalar pages retain complete schema and selected value qualification.
use super::{RecordProjection, TablePage, TablePageRequest, read_projected_records};
use crate::{InspectError, NativeReadSource, inspect_table_schema, table::snapshot};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Read an explicit ordinary scalar subset without loading unselected payloads.
/// Names use the existing ASCII-insensitive projection policy and values remain
/// in native logical column order. Missing/duplicate/unsupported selected fields
/// refuse before user rows, even when the requested page is empty.
///
/// Complete schema and allocation metadata stay visible and checked. Fetched
/// record framing and selected values are validated; skipped/unselected payloads
/// and an unvisited suffix remain outside this projection's qualification.
/// Only actual EOF verifies the declared count. No partial page is returned.
/// Empty selections produce zero-width rows. Use a stable/offline source.
pub fn read_table_projection_page<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    names: &[&str],
    request: TablePageRequest,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TablePage, InspectError> {
    let schema = inspect_table_schema(source, scope, table, budget, cancelled)?;
    let (selected, columns) = snapshot::projected_columns(&schema, names, budget, cancelled)?;
    let records = read_projected_records(
        source,
        &schema,
        RecordProjection {
            columns: &columns,
            selected: Some(&selected),
        },
        request,
        budget,
        cancelled,
    )?;
    Ok(TablePage {
        schema,
        columns,
        rows: records.rows,
        request,
        reached_end: records.reached_end,
    })
}
