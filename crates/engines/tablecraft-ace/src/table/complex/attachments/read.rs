use super::{AttachmentColumn, record, schema};
use crate::{
    InspectError, TableSnapshotLimits, header::check, read_complex_column_links_with_limits,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

/// Read complete ACE 2007/16-family stored attachment collections on a stable/offline
/// source. Metadata and scoped references are checked before publication. Native
/// wrapped bytes remain encoded; no file, URL or active content is opened.
pub fn read_attachment_column<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentColumn, InspectError> {
    read_attachment_column_with_limits(
        source,
        scope,
        table,
        column,
        TableSnapshotLimits::default(),
        budget,
        cancelled,
    )
}

/// Read a complete typed collection under explicit parent/child snapshot limits.
/// Existing schema, reference, per-parent and payload validation still applies.
pub fn read_attachment_column_with_limits<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    limits: TableSnapshotLimits,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.attachments").entered();
    let links = read_complex_column_links_with_limits(
        source, scope, table, column, limits, budget, cancelled,
    )?;
    read_links(links, budget, cancelled)
}

pub(in crate::table::complex) fn read_links(
    links: crate::ComplexColumnLinks,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentColumn, InspectError> {
    if !matches!(
        links.schema().owner().header.read_profile()?,
        crate::ReadProfile::Ace2007 | crate::ReadProfile::Ace2016 | crate::ReadProfile::Ace2019
    ) {
        return Err(InspectError::Unsupported);
    }
    let columns = schema::read(&links, budget, cancelled)?;
    for row in links.child_snapshot().rows() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, schema::FIELD_COUNT as u64)?;
        record::read(row, &columns)?;
    }
    check(budget, cancelled)?;
    Ok(AttachmentColumn { links, columns })
}
