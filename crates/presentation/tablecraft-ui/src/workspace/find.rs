//! Modeless native-table Find. Search and matching belong to the shared runtime.
mod dialog;
mod occurrence;
mod replace;
pub use replace::ReplaceFindRequest;

use super::filtering::{FilterPublication, FilterTarget};
use super::{Data, DocumentView, State};
use crate::CellKind;
use tablecraft_display::Options;

const MAX_FIND_PATTERN_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindTarget {
    pub document_revision: u64,
    pub table: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindSpan {
    pub start_utf16: u32,
    pub end_utf16: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindPosition {
    pub parent_ordinal: u32,
    pub column: String,
    pub span: Option<FindSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindScope {
    Field(String),
    Table,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FindMatch {
    Any,
    #[default]
    Whole,
    Start,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FindDirection {
    #[default]
    All,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindRequest {
    pub target: FindTarget,
    pub scope: FindScope,
    pub pattern: String,
    pub matching: FindMatch,
    pub direction: FindDirection,
    pub start: Option<FindPosition>,
    pub match_case: bool,
    pub formatted: bool,
    pub format_options: Options,
}

pub(super) struct Session {
    target: FindTarget,
    field: String,
    publication: u64,
    filter: Option<FilterWitness>,
    pattern: String,
    whole_table: bool,
    matching: FindMatch,
    direction: FindDirection,
    match_case: bool,
    formatted: bool,
    pending: Option<Pending>,
    found: Option<FindPosition>,
    found_request: Option<FindRequest>,
    replacement: replace::Replacement,
    not_found: bool,
    error: Option<String>,
    focus_input: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct FilterWitness {
    target: FilterTarget,
    enabled: bool,
}

enum Publication {
    Native(Data),
    Filtered(FilterPublication),
}

impl Publication {
    fn data(&self) -> &Data {
        match self {
            Self::Native(data) => data,
            Self::Filtered(publication) => &publication.data,
        }
    }
}

struct Pending {
    request_id: u64,
    request: FindRequest,
    publication: u64,
    filter: Option<FilterWitness>,
}

impl State {
    pub(in crate::workspace) fn find_from_record_search(
        &mut self,
        pattern: String,
    ) -> Option<super::Action> {
        if pattern.is_empty() || pattern.len() > MAX_FIND_PATTERN_BYTES || !self.can_open_find() {
            return None;
        }
        self.open_find();
        let session = self.find.as_mut()?;
        session.pattern = pattern;
        session.found = None;
        session.found_request = None;
        session.not_found = false;
        session.error = None;
        let request = self.current_find_request()?;
        self.can_find_next(&request)
            .then_some(super::Action::FindNext { request })
    }

    fn find_filter_witness(&self) -> Option<FilterWitness> {
        Some(FilterWitness {
            target: self.active_filter_target()?,
            enabled: self.table_filter_enabled(),
        })
    }

    fn find_source(&self) -> Option<(FindTarget, FindPosition)> {
        if self.total_selection_active()
            || self.view != DocumentView::Table
            || self.backstage_visible()
        {
            return None;
        }
        let data = self.data.as_ref()?;
        if data.row_ordinals.is_some() && self.find_filter_witness().is_none() {
            return None;
        }
        let revision = data.source_revision?;
        if self.database.as_ref()?.document_revision != revision {
            return None;
        }
        let (row, column) = self.table_selection.position()?;
        let cell = data.rows.get(row)?.get(column)?;
        if !matches!(cell.kind, CellKind::Scalar { .. }) {
            return None;
        }
        Some((
            FindTarget {
                document_revision: revision,
                table: data.table.clone(),
            },
            FindPosition {
                parent_ordinal: data.parent_ordinal(row)?,
                column: data.columns.get(column)?.name.clone(),
                span: None,
            },
        ))
    }

    pub fn can_open_find(&self) -> bool {
        (self.find_available || self.mock_preview)
            && !self.busy
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && !self.table_filter_pending()
            && self.find_source().is_some()
    }

    pub fn open_find(&mut self) {
        if !self.can_open_find() {
            return;
        }
        if let Some(session) = &mut self.find {
            session.replacement.visible = false;
            session.focus_input = true;
            return;
        }
        let (target, position) = self.find_source().expect("admitted native selection");
        self.find = Some(Session {
            target,
            field: position.column,
            publication: self.focus_publication,
            filter: self.find_filter_witness(),
            pattern: String::new(),
            whole_table: false,
            matching: FindMatch::default(),
            direction: FindDirection::default(),
            match_case: false,
            formatted: false,
            pending: None,
            found: None,
            found_request: None,
            replacement: replace::Replacement::default(),
            not_found: false,
            error: None,
            focus_input: true,
        });
    }

    pub(super) fn prepare_mock_find(&mut self, request_id: Option<u64>) {
        if !self.mock_preview {
            return;
        }
        self.open_find();
        let Some(session) = &mut self.find else {
            return;
        };
        session.pattern = "Customer*".into();
        if let Some(request_id) = request_id
            && let Some(request) = self.current_find_request()
        {
            self.begin_find(request_id, request);
        }
    }

    pub(super) fn prepare_mock_replace(&mut self, error: bool) {
        if !self.mock_preview {
            return;
        }
        self.prepare_mock_find(None);
        if let Some(session) = &mut self.find {
            session.replacement.visible = true;
            session.replacement.text = "Updated customer".into();
            if error {
                session.error = Some(self.localizer.text("find-replace-mock-help"));
            }
        }
    }

    pub fn find_window_open(&self) -> bool {
        self.find.is_some()
    }

    pub(super) fn current_find_request(&self) -> Option<FindRequest> {
        let session = self.find.as_ref()?;
        let (target, mut position) = self.find_source()?;
        if target != session.target
            || session.publication != self.focus_publication
            || session.filter != self.find_filter_witness()
        {
            return None;
        }
        if let Some(found) = &session.found
            && found.parent_ordinal == position.parent_ordinal
            && found.column == position.column
        {
            position.span = found.span.clone();
        }
        Some(FindRequest {
            target,
            scope: if session.whole_table {
                FindScope::Table
            } else {
                FindScope::Field(position.column.clone())
            },
            pattern: session.pattern.clone(),
            matching: session.matching,
            direction: session.direction,
            start: Some(position),
            match_case: session.match_case,
            formatted: session.formatted,
            format_options: self.formatting.clone(),
        })
    }

    pub fn can_find_next(&self, request: &FindRequest) -> bool {
        self.can_open_find()
            && self.find.as_ref().is_some_and(|session| {
                session.pending.is_none() && session.replacement.pending.is_none()
            })
            && !request.pattern.is_empty()
            && request.pattern.len() <= MAX_FIND_PATTERN_BYTES
            && (!request.formatted || self.find_formatted_available || self.mock_preview)
            && self.current_find_request().as_ref() == Some(request)
    }

    pub fn begin_find(&mut self, request_id: u64, request: FindRequest) {
        if !self.can_find_next(&request) {
            return;
        }
        let filter = self.find_filter_witness();
        let session = self.find.as_mut().expect("admitted Find session");
        session.pending = Some(Pending {
            request_id,
            request,
            publication: self.focus_publication,
            filter,
        });
        session.error = None;
        session.not_found = false;
    }

    pub fn accepts_find_reply(&self, request_id: u64, request: &FindRequest) -> bool {
        !self.edit_dialog_open()
            && !self.table_filter_pending()
            && !self.zoom_dialog_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && self.current_find_request().as_ref() == Some(request)
            && self.find.as_ref().is_some_and(|session| {
                session.pending.as_ref().is_some_and(|pending| {
                    pending.request_id == request_id
                        && pending.request == *request
                        && pending.publication == self.focus_publication
                        && pending.filter == self.find_filter_witness()
                })
            })
    }

    pub fn set_find_result(
        &mut self,
        request_id: u64,
        request: FindRequest,
        found: Option<FindPosition>,
        page: Option<Data>,
    ) -> bool {
        if self.find_filter_witness().is_some() {
            return false;
        }
        self.publish_find_result(request_id, request, found, page.map(Publication::Native))
    }

    pub fn set_filtered_find_result(
        &mut self,
        request_id: u64,
        request: FindRequest,
        found: Option<FindPosition>,
        page: Option<FilterPublication>,
    ) -> bool {
        if self.find_filter_witness().is_none() {
            return false;
        }
        self.publish_find_result(request_id, request, found, page.map(Publication::Filtered))
    }

    fn publish_find_result(
        &mut self,
        request_id: u64,
        request: FindRequest,
        found: Option<FindPosition>,
        page: Option<Publication>,
    ) -> bool {
        if !self.accepts_find_reply(request_id, &request) {
            return false;
        }
        match (found, page) {
            (None, None) => {
                let session = self.find.as_mut().expect("accepted Find session");
                session.pending = None;
                session.not_found = true;
                session.found = None;
                session.found_request = None;
                true
            }
            (Some(found), Some(publication)) => {
                let data = publication.data();
                let position = data.row_index(found.parent_ordinal).and_then(|row| {
                    let column = data.column_index(&found.column)?;
                    data.rows.get(row)?.get(column)?;
                    Some((row, column))
                });
                let valid_span = found
                    .span
                    .as_ref()
                    .is_some_and(|span| span.start_utf16 <= span.end_utf16);
                let matches_scope = match &request.scope {
                    FindScope::Field(column) => *column == found.column,
                    FindScope::Table => true,
                };
                let matches_view = match &publication {
                    Publication::Native(data) => {
                        data.row_ordinals.is_none()
                            && data.offset == found.parent_ordinal
                            && data.parent_ordinal(0) == Some(found.parent_ordinal)
                    }
                    Publication::Filtered(publication) => self.accepts_filtered_view(publication),
                };
                let valid = valid_span
                    && matches_scope
                    && matches_view
                    && position.is_some()
                    && data.source_revision == Some(request.target.document_revision)
                    && data.table == request.target.table;
                if !valid {
                    self.fail_find(request_id, self.localizer.text("find-invalid-result"));
                    return false;
                }

                let mut session = self.find.take().expect("accepted Find session");
                let accepted = match publication {
                    Publication::Native(data) => self.set_table_data(data),
                    Publication::Filtered(publication) => self.install_filtered_view(publication),
                };
                if !accepted {
                    self.find = Some(session);
                    self.fail_find(request_id, self.localizer.text("objects-memory-limit"));
                    return false;
                }
                let (row, column) = position.expect("validated exact field and member");
                self.table_selection.select(
                    self.data.as_ref().expect("published Find page"),
                    row,
                    column,
                );
                session.pending = None;
                session.publication = self.focus_publication;
                session.field = found.column.clone();
                session.found = Some(found);
                session.found_request = Some(request);
                session.not_found = false;
                self.find = Some(session);
                true
            }
            _ => {
                self.fail_find(request_id, self.localizer.text("find-invalid-result"));
                false
            }
        }
    }

    pub fn fail_find(&mut self, request_id: u64, message: String) {
        let accepts = self
            .find
            .as_ref()
            .and_then(|session| session.pending.as_ref())
            .is_some_and(|pending| self.accepts_find_reply(request_id, &pending.request));
        if accepts && let Some(session) = &mut self.find {
            session.pending = None;
            session.found = None;
            session.found_request = None;
            session.error = Some(message);
        }
    }

    /// Closing requests cancellation of the current read; it never changes rows.
    pub fn close_find(&mut self) -> Option<u64> {
        let replace = self.replace_request_id();
        self.find
            .take()?
            .pending
            .map(|pending| pending.request_id)
            .or(replace)
    }
}

#[cfg(test)]
mod tests;
