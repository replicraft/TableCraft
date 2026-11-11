//! Child inspection reads only the immutable issued metadata, never native pages.
use super::QueryResultHandle;
use crate::{
    Cell, CellKind, Error, HorizontalAlignment, Response, display::Presentation, document::Document,
};
use std::sync::Arc;
use tablecraft_db::StoredCell;

impl Document {
    pub(crate) fn inspect_query_collection(
        &self,
        result: &QueryResultHandle,
        binding: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if result.state.revision != self.revision
            || !Arc::ptr_eq(&result.state.source, &self.filter_source)
        {
            return Err(Error::StaleDocument);
        }
        let mut output = Presentation::new(cancelled);
        output.work(1)?;
        let recordset = &result
            .state
            .bindings
            .get(binding)
            .ok_or(Error::QueryContentsUnavailable)?
            .value;
        let mut columns = output.columns(recordset.schema().columns().len())?;
        for column in recordset.schema().columns() {
            columns.push(crate::column::plain(column.name(), &mut output)?);
        }
        let mut rows = output.rows(recordset.len())?;
        for stored in recordset.rows() {
            let mut row = output.row(stored.len())?;
            for value in stored {
                row.push(match value {
                    StoredCell::Value(value) => output.scalar(value, false)?,
                    StoredCell::Uninspected => Cell {
                        presentation: None,
                        alignment: HorizontalAlignment::Left,
                        text: output.text("Not inspected")?,
                        kind: CellKind::Uninspected,
                    },
                });
            }
            rows.push(row);
        }
        Ok(Response::QueryContents {
            binding,
            columns,
            rows,
        })
    }
}
