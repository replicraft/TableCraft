//! Bounded presentation of explicitly selected rows from one qualified native page.
use super::{collections, controls, formatting};
use crate::{Cell, Column, Error, display::Presentation};
use tablecraft_access::BrowsePage;
use tablecraft_ace::{InspectError, TableProperties};

pub(crate) struct PagePresentation {
    pub(crate) columns: Vec<Column>,
    pub(crate) rows: Vec<Vec<Cell>>,
    pub(crate) reached_end: bool,
}

pub(crate) fn present(
    page: &BrowsePage,
    properties: Option<&TableProperties>,
    selected: Option<&[usize]>,
    output: &mut Presentation<'_>,
) -> Result<PagePresentation, Error> {
    let (columns, mut rows, reached_end) = match page {
        BrowsePage::Scalar(page) => {
            output.work(1)?;
            crate::native_properties::require_binding(properties, page.schema())?;
            let mut columns = output.columns(page.columns().len())?;
            for column in page.columns() {
                columns.push(crate::column::native(column.name(), properties, output)?);
            }
            let count = selected.map_or(page.rows().len(), <[usize]>::len);
            let mut rows = output.rows(count)?;
            for index in 0..count {
                output.work(1)?;
                let parent = selected.map_or(index, |selected| selected[index]);
                let values = page.rows().get(parent).ok_or(InspectError::Malformed)?;
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
            crate::native_properties::require_binding(properties, page.schema())?;
            let mut columns = output.columns(page.schema().columns().len())?;
            for column in page.schema().columns() {
                columns.push(crate::column::native(column.name(), properties, output)?);
            }
            let rows = collections::rows(page, selected, output)?;
            (columns, rows, page.reached_end())
        }
    };
    controls::enrich(properties, &columns, &mut rows, output)?;
    formatting::apply(properties, &columns, &mut rows, output)?;
    Ok(PagePresentation {
        columns,
        rows,
        reached_end,
    })
}
