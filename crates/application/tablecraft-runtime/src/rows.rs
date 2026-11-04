//! Bounded scalar pages from the retained immutable session image.
use super::{
    Error, Response,
    controls::{SCOPE, budget},
    display::Presentation,
    document::Document,
};
use tablecraft_access::NativeBrowser;

const PAGE_ROWS: u32 = 100;

impl Document {
    pub(super) fn page(
        &mut self,
        table: &str,
        offset: u32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, budget(), cancelled);
        let request = tablecraft_access::TablePageRequest::new(offset, PAGE_ROWS)?;
        let page = browser.page(table, request)?;
        let mut output = Presentation::new(cancelled);
        let columns = page
            .columns()
            .iter()
            .map(|column| output.text(column.name()))
            .collect::<Result<_, _>>()?;
        let rows = page
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| output.value(value))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Response::Data {
            table: table.to_owned(),
            offset,
            columns,
            rows,
            has_more: !page.reached_end(),
        })
    }
}
