//! Metadata-selected scalar/stored-field pages from the immutable read session.
mod collections;
mod controls;

use super::{
    Error, Response,
    controls::{SCOPE, budget},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{BrowsePage, CollectionPageRequest, NativeBrowser, TablePageRequest};
use tablecraft_ace::TableSnapshotLimits;

const PAGE_ROWS: u32 = 100;

impl Document {
    pub(super) fn page(
        &mut self,
        table: &str,
        offset: u32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let mut work = budget();
        let properties = controls::inspect(&mut self.source, table, &mut work, cancelled)?;
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, work, cancelled);
        let request = CollectionPageRequest::new(
            TablePageRequest::new(offset, PAGE_ROWS)?,
            TableSnapshotLimits::default(),
        );
        // Metadata selects exactly one existing reader. Failures are never
        // retried as another profile or converted to a partial/empty collection.
        let page = browser.browse_page(table, request)?;
        let mut output = Presentation::new(cancelled);
        let (columns, mut rows, reached_end) = match page {
            BrowsePage::Scalar(page) => {
                output.work(1)?;
                controls::require_binding(properties.as_ref(), page.schema())?;
                let mut columns = output.columns(page.columns().len())?;
                for column in page.columns() {
                    columns.push(output.text(column.name())?);
                }
                let mut rows = output.rows(page.rows().len())?;
                for values in page.rows() {
                    let mut row = output.row(values.len())?;
                    for value in values {
                        row.push(output.scalar(value, false)?);
                    }
                    rows.push(row);
                }
                (columns, rows, page.reached_end())
            }
            BrowsePage::Collections(page) => {
                output.work(1)?;
                controls::require_binding(properties.as_ref(), page.schema())?;
                let mut columns = output.columns(page.schema().columns().len())?;
                for column in page.schema().columns() {
                    columns.push(output.text(column.name())?);
                }
                let rows = collections::rows(&page, &mut output)?;
                (columns, rows, page.reached_end())
            }
        };
        controls::enrich(properties.as_ref(), &columns, &mut rows, &mut output)?;
        Ok(Response::Data {
            source_revision: Some(self.revision),
            table: output.text(table)?,
            offset,
            columns,
            rows,
            has_more: !reached_end,
        })
    }
}
