use super::{VersionHistoryColumn, record, schema};
use crate::{InspectError, header::check, inspect_complex_schema};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const RECORD_FIELDS: u64 = 3;

/// Read stored versions for one explicitly named visible Memo on a stable/offline
/// ACE 16 source. Resolve its hidden history key without assuming generated names.
/// Preserve exact stored order/text/DATE bits; no latest-value selection, history
/// update/delete, application callback or active content executes.
pub fn read_version_history_column<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    memo: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<VersionHistoryColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.history").entered();
    check(budget, cancelled)?;
    let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    let (position, memo_position) = schema::select(&schema, memo, budget, cancelled)?;
    let links =
        super::super::links::read_resolved(source, scope, schema, position, budget, cancelled)?;
    let columns = schema::read(&links)?;
    for row in links.child_snapshot().rows() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, RECORD_FIELDS)?;
        record::read(row, &columns)?;
    }
    check(budget, cancelled)?;
    Ok(VersionHistoryColumn {
        links,
        memo_position,
        columns,
    })
}
