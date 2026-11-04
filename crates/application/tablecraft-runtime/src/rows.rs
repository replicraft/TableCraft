//! Metadata-selected scalar/stored-field pages from the immutable read session.
mod collections;

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
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, budget(), cancelled);
        let request = CollectionPageRequest::new(
            TablePageRequest::new(offset, PAGE_ROWS)?,
            TableSnapshotLimits::default(),
        );
        // Metadata selects exactly one existing reader. Failures are never
        // retried as another profile or converted to a partial/empty collection.
        let page = browser.browse_page(table, request)?;
        let mut output = Presentation::new(cancelled);
        let (columns, rows, reached_end) = match page {
            BrowsePage::Scalar(page) => {
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
                let mut columns = output.columns(page.schema().columns().len())?;
                for column in page.schema().columns() {
                    columns.push(output.text(column.name())?);
                }
                let rows = collections::rows(&page, &mut output)?;
                (columns, rows, page.reached_end())
            }
        };
        Ok(Response::Data {
            table: output.text(table)?,
            offset,
            columns,
            rows,
            has_more: !reached_end,
        })
    }
}
