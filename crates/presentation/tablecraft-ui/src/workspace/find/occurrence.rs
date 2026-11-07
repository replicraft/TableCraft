//! A current native Find receipt supplies paint-only, page-local coordinates.
use super::{FindMatch, FindPosition, FindSpan, State};
use crate::Data;
use crate::datasheet::occurrence::Occurrence;

impl State {
    pub(in crate::workspace) fn prepare_mock_find_occurrence(
        &mut self,
        request_id: u64,
        data: Data,
        start_utf16: u32,
    ) {
        if !self.mock_preview {
            return;
        }
        self.open_find();
        let session = self.find.as_mut().expect("synthetic native table");
        session.pattern = "🦀".into();
        session.matching = FindMatch::Any;
        let request = self.current_find_request().expect("synthetic Find");
        self.begin_find(request_id, request.clone());
        assert!(self.set_find_result(
            request_id,
            request,
            Some(FindPosition {
                parent_ordinal: 0,
                column: "Notes".into(),
                span: Some(FindSpan {
                    start_utf16,
                    end_utf16: start_utf16 + 2,
                }),
            }),
            Some(data),
        ));
    }

    pub(crate) fn current_find_occurrence(&self) -> Option<Occurrence> {
        if !self.found_receipt_current() {
            return None;
        }
        let session = self.find.as_ref()?;
        let found = session.found.as_ref()?;
        let request = session.found_request.as_ref()?;
        let span = found.span.as_ref()?;
        let data = self.data.as_ref()?;

        Some(Occurrence {
            row: data.row_index(found.parent_ordinal)?,
            column: data.column_index(&found.column)?,
            start_utf16: span.start_utf16,
            end_utf16: span.end_utf16,
            formatted: request.formatted,
        })
    }
}

#[cfg(test)]
mod tests;
