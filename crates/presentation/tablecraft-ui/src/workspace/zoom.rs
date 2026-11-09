//! Source-bound, read-only enlargement; editing delegates to the existing review.
mod dialog;
mod index;

use super::{DocumentView, EditTarget, State};
use crate::{Cell, CellKind};
use index::{Index, IndexError, Visible};
use tablecraft_display::Options;

const DEFAULT_TEXT_SIZE: u8 = 14;

pub(super) struct Zoom {
    session: Option<Session>,
    text_size: u8,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            session: None,
            text_size: DEFAULT_TEXT_SIZE,
        }
    }
}

struct Binding {
    publication: u64,
    view: DocumentView,
    saved_query: bool,
    document_revision: Option<u64>,
    source_revision: Option<u64>,
    parent_ordinal: Option<u32>,
    table: String,
    offset: u32,
    row: usize,
    column: usize,
    pointer: usize,
    bytes: usize,
    formatting: Options,
}

struct Session {
    binding: Binding,
    index: Index,
    visible: Visible,
    focus_body: bool,
    pending_scroll: Option<f32>,
    pending_horizontal: Option<f32>,
    scroll_offset: f32,
    horizontal_offset: f32,
    max_width: f32,
    copy_error: bool,
}

impl Binding {
    fn cell<'a>(&self, state: &'a State) -> Option<&'a Cell> {
        if state.backstage_visible()
            || state.zoom_position() != Some((self.row, self.column))
            || state.focus_publication != self.publication
            || state.view != self.view
            || state.saved_query.active != self.saved_query
            || state
                .database
                .as_ref()
                .map(|database| database.document_revision)
                != self.document_revision
            || state.formatting != self.formatting
        {
            return None;
        }
        let data = state.visible_data()?;
        if data.parent_ordinal(self.row) != self.parent_ordinal
            || data.source_revision != self.source_revision
            || data.table != self.table
            || data.offset != self.offset
        {
            return None;
        }
        let cell = data.rows.get(self.row)?.get(self.column)?;
        (matches!(cell.kind, CellKind::Scalar { .. })
            && cell.text.len() == self.bytes
            && cell.text.as_ptr() as usize == self.pointer)
            .then_some(cell)
    }
}

pub(crate) fn can_view(cell: &Cell) -> bool {
    matches!(cell.kind, CellKind::Scalar { .. }) && cell.text.len() <= index::MAX_SOURCE_BYTES
}

impl State {
    fn zoom_position(&self) -> Option<(usize, usize)> {
        if self.total_selection_active() {
            return None;
        }
        let selection = match self.view {
            DocumentView::Table => &self.table_selection,
            DocumentView::Query if self.saved_query.active => &self.saved_query.selection,
            DocumentView::Query => &self.query_selection,
            DocumentView::Design => return None,
        };
        (!selection.row_selected())
            .then(|| selection.position())
            .flatten()
    }

    pub fn can_zoom_cell(&self) -> bool {
        if self.busy
            || self.backstage_visible()
            || self.edit_dialog_open()
            || self.contents_request_id().is_some()
            || self.parameter_prompt_open()
            || self.zoom_dialog_open()
        {
            return false;
        }
        let Some((row, column)) = self.zoom_position() else {
            return false;
        };
        self.visible_data()
            .and_then(|data| data.rows.get(row)?.get(column))
            .is_some_and(can_view)
    }

    /// Enlarges the complete displayed scalar. No parser, database or writer runs.
    pub fn open_zoom_cell(&mut self) -> bool {
        if !self.can_zoom_cell() {
            return false;
        }
        let (row, column) = self.zoom_position().expect("admitted scalar selection");
        let data = self.visible_data().expect("admitted data publication");
        let cell = &data.rows[row][column];
        let index = match Index::build(&cell.text) {
            Ok(index) => index,
            Err(error) => {
                self.error = Some(self.localizer.text(match error {
                    IndexError::Limit => "zoom-limit",
                    IndexError::Allocation | IndexError::Changed => "zoom-unavailable",
                }));
                return false;
            }
        };
        let binding = Binding {
            publication: self.focus_publication,
            view: self.view,
            saved_query: self.saved_query.active,
            document_revision: self
                .database
                .as_ref()
                .map(|database| database.document_revision),
            source_revision: data.source_revision,
            parent_ordinal: data.parent_ordinal(row),
            // Retain the exact prior source, selected column, scope, pattern or publication
            // binding across mutation/publication. These owners reject stale work or preserve
            // the other input; they cannot borrow the mutated State.
            table: data.table.clone(),
            offset: data.offset,
            row,
            column,
            pointer: cell.text.as_ptr() as usize,
            bytes: cell.text.len(),
            // Capture the actual formatting policy independently for cache invalidation,
            // pending-result rejection or the displayed dialog binding; later preference edits
            // must not mutate that snapshot.
            formatting: self.formatting.clone(),
        };
        self.zoom.session = Some(Session {
            binding,
            index,
            visible: Visible::default(),
            focus_body: true,
            pending_scroll: Some(0.0),
            pending_horizontal: Some(0.0),
            scroll_offset: 0.0,
            horizontal_offset: 0.0,
            max_width: 0.0,
            copy_error: false,
        });
        true
    }

    pub fn zoom_dialog_open(&self) -> bool {
        self.zoom.session.is_some()
    }

    pub fn dismiss_zoom(&mut self) {
        self.zoom.session = None;
    }

    pub(super) fn validate_zoom(&mut self) {
        if self
            .zoom
            .session
            .as_ref()
            .is_some_and(|session| self.busy || session.binding.cell(self).is_none())
        {
            self.dismiss_zoom();
        }
    }

    fn restore_zoom_focus(&mut self, context: &egui::Context, binding: &Binding) {
        if binding.cell(self).is_none() {
            return;
        }
        match self.view {
            DocumentView::Table => self.table_selection.focus_selected(context),
            DocumentView::Query if self.saved_query.active => {
                self.saved_query.selection.focus_selected(context)
            }
            DocumentView::Query => self.query_selection.focus_selected(context),
            DocumentView::Design => {}
        }
    }

    fn zoom_edit_target(&self, binding: &Binding) -> Option<EditTarget> {
        if self.view != DocumentView::Table {
            return None;
        }
        let data = self.visible_data()?;
        data.edit_target(binding.row, binding.column)
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default)]
struct Request {
    frame: u64,
    position: (usize, usize),
    pointer: usize,
    bytes: usize,
}

fn request_id() -> egui::Id {
    egui::Id::new("zoom_cell_request")
}

pub(crate) fn request(context: &egui::Context, cell: &Cell, position: (usize, usize)) {
    let request = Request {
        frame: context.cumulative_frame_nr(),
        position,
        pointer: cell.text.as_ptr() as usize,
        bytes: cell.text.len(),
    };
    context.data_mut(|data| data.insert_temp(request_id(), request));
}

impl State {
    pub(super) fn accept_zoom_request(&mut self, context: &egui::Context) {
        let Some(request) = context.data_mut(|data| data.remove_temp::<Request>(request_id()))
        else {
            return;
        };
        if request.frame != context.cumulative_frame_nr()
            || self.zoom_position() != Some(request.position)
        {
            return;
        }
        let current = self
            .visible_data()
            .and_then(|data| data.rows.get(request.position.0)?.get(request.position.1));
        if current.is_some_and(|cell| {
            cell.text.as_ptr() as usize == request.pointer && cell.text.len() == request.bytes
        }) {
            self.open_zoom_cell();
        }
    }
}
