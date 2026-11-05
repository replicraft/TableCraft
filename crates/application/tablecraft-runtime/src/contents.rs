//! Explicit complete contents from one source-local stored collection cell.
mod items;

use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{
    BrowsePage, CollectionPageRequest, NativeBrowser, StoredCollectionColumn,
    StoredCollectionPageValue, TablePageRequest,
};
use tablecraft_ace::{InspectError, TableSchema, TableSnapshotLimits};

/// A staleness precondition scoped to one runtime worker's immutable document.
/// This is not a physical ROW, durable bookmark, or authorization token.
#[derive(Clone, PartialEq, Eq)]
pub struct ContentsTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub parent_ordinal: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionKind {
    Multivalue,
    Attachments,
    VersionHistory,
}

impl Document {
    pub(super) fn inspect_collection(
        &mut self,
        target: &ContentsTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        check(cancelled)?;
        let mut output = Presentation::new(cancelled);
        output.work(target.table.len() as u64)?;
        output.work(target.column.len() as u64)?;
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, budget(), cancelled);
        let request = CollectionPageRequest::new(
            TablePageRequest::new(target.parent_ordinal, 1)?,
            TableSnapshotLimits::default(),
        );
        // This intentionally reuses complete qualified retention. It does not
        // promise lazy child I/O or decode/extract/open an attachment payload.
        let page = browser.browse_page(&target.table, request)?;
        let page = match page {
            BrowsePage::Scalar(page) => {
                if page.rows().is_empty() {
                    return Err(Error::CollectionParentMissing);
                }
                selected_column(page.schema(), target, &mut output)?;
                return Err(Error::NotCollection);
            }
            BrowsePage::Collections(page) => page,
        };
        if page.row_count() != 1 {
            return Err(Error::CollectionParentMissing);
        }
        let position = selected_column(page.schema(), target, &mut output)?;
        output.work(page.schema().columns().len() as u64)?;
        let collection = page
            .collection_column(position)
            .ok_or(Error::NotCollection)?;
        let kind = match collection {
            StoredCollectionColumn::Multivalue(_) => CollectionKind::Multivalue,
            StoredCollectionColumn::Attachments(_) => CollectionKind::Attachments,
            StoredCollectionColumn::VersionHistory(_) => CollectionKind::VersionHistory,
        };
        output.work(page.schema().columns().len() as u64)?;
        let cell = match page.cell(0, position).ok_or(InspectError::Malformed)? {
            StoredCollectionPageValue::Collection(cell) => cell,
            StoredCollectionPageValue::Scalar(_) => return Err(Error::NotCollection),
        };
        let assigned = cell.key().is_some();
        let mut columns = output.columns(items::headers(kind).len())?;
        for name in items::headers(kind) {
            columns.push(crate::column::plain(name, &mut output)?);
        }
        let children = cell.items()?;
        let mut rows = output.rows(children.len())?;
        for child in children {
            output.work(1)?;
            rows.push(items::row(kind, child?, &mut output)?);
        }
        if !assigned && !rows.is_empty() {
            return Err(InspectError::Malformed.into());
        }
        let target = ContentsTarget {
            document_revision: self.revision,
            table: output.text(&target.table)?,
            column: output.text(&target.column)?,
            parent_ordinal: target.parent_ordinal,
        };
        output.work(1)?;
        Ok(Response::Contents {
            target,
            kind,
            assigned,
            columns,
            rows,
        })
    }
}

fn selected_column(
    schema: &TableSchema,
    target: &ContentsTarget,
    output: &mut Presentation<'_>,
) -> Result<usize, Error> {
    output.work(schema.name().len() as u64)?;
    output.work(target.table.len() as u64)?;
    if schema.name() != target.table {
        return Err(Error::CollectionColumnMissing);
    }
    for (position, field) in schema.columns().iter().enumerate() {
        output.work(1)?;
        output.work(field.name().len() as u64)?;
        output.work(target.column.len() as u64)?;
        if field.name() == target.column {
            return Ok(position);
        }
    }
    Err(Error::CollectionColumnMissing)
}
