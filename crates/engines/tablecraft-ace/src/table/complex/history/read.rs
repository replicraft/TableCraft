use super::{VersionHistoryColumn, record, schema};
use crate::{InspectError, TableSnapshotLimits, header::check, inspect_complex_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const RECORD_FIELDS: u64 = 3;

/// Read stored versions for one explicitly named visible Memo on a stable/offline
/// ACE 2007/16 source. Resolve its hidden history key without assuming generated names.
/// Preserve exact stored order/text/DATE bits; no latest-value selection, history
/// update/delete, application callback or active content executes.
pub fn read_version_history_column<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    memo: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<VersionHistoryColumn, InspectError> {
    read_version_history_column_with_limits(
        source,
        scope,
        table,
        memo,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

/// Read complete stored versions under explicit parent/child snapshot limits.
/// Exact stored order and DATE bits remain unchanged; no history is executed.
pub fn read_version_history_column_with_limits<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    memo: &str,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<VersionHistoryColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.history").entered();
    check(budget, cancelled)?;
    let schema = inspect_complex_schema(source, scope, table, budget, cancelled)?;
    if !matches!(
        schema.owner().header.read_profile()?,
        crate::ReadProfile::Ace2007 | crate::ReadProfile::Ace2016 | crate::ReadProfile::Ace2019
    ) {
        return Err(InspectError::Unsupported);
    }
    let (position, memo_position) = schema::select(&schema, memo, budget, cancelled)?;
    let links = super::super::links::read_resolved(
        source, scope, schema, position, limits, budget, cancelled,
    )?;
    read_links(links, memo_position, budget, cancelled)
}

pub(in crate::table::complex) fn read_links(
    links: crate::ComplexColumnLinks,
    memo_position: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<VersionHistoryColumn, InspectError> {
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
