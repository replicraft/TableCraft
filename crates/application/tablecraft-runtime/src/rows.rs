//! Metadata-selected scalar/stored-field pages from the immutable read session.
mod collections;
mod controls;
mod formatting;
pub(crate) mod page;
pub(crate) mod presentation;

use super::{
    Error, Response,
    controls::{SCOPE, budget},
    display::Presentation,
    document::Document,
};
use page::PagePresentation;
use tablecraft_access::{CollectionPageRequest, NativeBrowser, TablePageRequest};
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
        let properties =
            crate::native_properties::inspect(&mut self.source, table, &mut work, cancelled)?;
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, work, cancelled);
        let request = CollectionPageRequest::new(
            TablePageRequest::new(offset, PAGE_ROWS)?,
            TableSnapshotLimits::default(),
        );
        // Metadata selects exactly one existing reader. Failures are never
        // retried as another profile or converted to a partial/empty collection.
        let page = browser.browse_page(table, request)?;
        let mut output = Presentation::new(cancelled);
        let PagePresentation {
            columns,
            rows,
            reached_end,
        } = page::present(&page, properties.as_ref(), None, &mut output)?;
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
