use super::{AttachmentColumn, record, schema};
use crate::{InspectError, header::check, read_complex_column_links};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

/// Read complete ACE 16-family stored attachment collections on a stable/offline
/// source. Metadata and scoped references are checked before publication. Native
/// wrapped bytes remain encoded; no file, URL or active content is opened.
pub fn read_attachment_column<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AttachmentColumn, InspectError> {
    let _span = tracing::debug_span!("native.complex.attachments").entered();
    let links = read_complex_column_links(source, scope, table, column, budget, cancelled)?;
    if !matches!(
        links.schema().owner().header.read_profile()?,
        crate::ReadProfile::Ace2016 | crate::ReadProfile::Ace2019
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
