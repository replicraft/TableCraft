//! Metadata-selected scalar/stored-field pages from the immutable read session.
mod collections;
mod controls;
mod formatting;
pub(crate) mod page;
pub(crate) mod presentation;
#[cfg(test)]
mod tests;

use super::{
    Error, Response,
    controls::{SCOPE, budget},
    display::Presentation,
    document::Document,
};
use page::PagePresentation;
use tablecraft_access::{BrowsePage, NativeBrowser};
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_ace::{CollectionPageRequest, TablePageRequest};

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
            TablePageRequest::new(offset, PAGE_ROWS + 1)?,
            TableSnapshotLimits::default(),
        );
        // Metadata selects exactly one existing reader. Failures are never
        // retried as another profile or converted to a partial/empty collection.
        let page = browser.browse_page(table, request)?;
        let count = match &page {
            BrowsePage::Scalar(page) => page.rows().len(),
            BrowsePage::Collections(page) => page.row_count(),
        };
        // Validate one additional native row under the same work budget, without
        // publishing it. A final full display page therefore reaches checked EOF
        // instead of advertising an empty next page. Declared counts are not used.
        let selected: [usize; PAGE_ROWS as usize] = std::array::from_fn(|index| index);
        let selected = &selected[..count.min(selected.len())];
        let mut output = Presentation::new(cancelled);
        let PagePresentation {
            columns,
            rows,
            reached_end,
        } = page::present(&page, properties.as_ref(), Some(selected), &mut output)?;
        Ok(Response::Data {
            query_contents: None,
            source_revision: Some(self.revision),
            table: output.text(table)?,
            offset,
            columns,
            rows,
            has_more: count > selected.len() || !reached_end,
        })
    }
}
