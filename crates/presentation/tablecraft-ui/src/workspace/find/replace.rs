//! Replace carries a successful native match into the existing reviewed edit service.
use super::{FilterWitness, FindPosition, FindRequest, State};
use crate::text_input::MAX_EDITOR_SCALARS;
use crate::workspace::editing::{EditDialog, EditDraft, EditTarget};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplaceFindRequest {
    pub find: FindRequest,
    pub found: FindPosition,
    pub replacement: String,
}

#[derive(Default)]
pub(super) struct Replacement {
    pub(super) visible: bool,
    pub(super) text: String,
    pub(super) pending: Option<Pending>,
}

pub(super) struct Pending {
    request_id: u64,
    request: ReplaceFindRequest,
    publication: u64,
    filter: Option<FilterWitness>,
}

fn edit_target(request: &ReplaceFindRequest) -> EditTarget {
    EditTarget {
        document_revision: request.find.target.document_revision,
        // NO_CLONE: Construct a typed owned target whose table/field identity is retained
        // separately from the borrowed data/catalog/query snapshot. The source
        // observation remains available after command dispatch.
        table: request.find.target.table.clone(),
        // NO_CLONE: Construct a typed owned target whose table/field identity is retained
        // separately from the borrowed data/catalog/query snapshot. The source
        // observation remains available after command dispatch.
        column: request.found.column.clone(),
        parent_ordinal: request.found.parent_ordinal,
    }
}

fn same_search(left: &FindRequest, right: &FindRequest) -> bool {
    left.target == right.target
        && left.scope == right.scope
        && left.pattern == right.pattern
        && left.matching == right.matching
        && left.direction == right.direction
        && left.match_case == right.match_case
        && left.formatted == right.formatted
        && left.format_options == right.format_options
}

impl State {
    pub fn open_replace(&mut self) {
        if !self.can_open_find() {
            return;
        }
        self.open_find();
        if let Some(session) = &mut self.find {
            session.replacement.visible = true;
        }
    }

    pub(super) fn found_receipt_current(&self) -> bool {
        let Some(session) = &self.find else {
            return false;
        };
        let (Some(found), Some(original), Some(current), Some((_, position))) = (
            session.found.as_ref(),
            session.found_request.as_ref(),
            self.current_find_request(),
            self.find_source(),
        ) else {
            return false;
        };
        same_search(original, &current)
            && position.parent_ordinal == found.parent_ordinal
            && position.column == found.column
            && session.publication == self.focus_publication
            && session.filter == self.find_filter_witness()
    }

    pub fn current_replace_find_request(&self) -> Option<ReplaceFindRequest> {
        if !self.found_receipt_current() {
            return None;
        }
        let session = self.find.as_ref()?;
        session.replacement.visible.then(|| ReplaceFindRequest {
            // NO_CLONE: ReplaceFindRequest needs its own captured identity after this
            // render/dispatch; keep the original catalog, input or owner available
            // for the later checks in this operation.
            find: session
                .found_request
                .as_ref()
                .expect("checked match receipt")
                .clone(),
            // NO_CLONE: ReplaceFindRequest needs its own captured identity after this
            // render/dispatch; keep the original catalog, input or owner available
            // for the later checks in this operation.
            found: session
                .found
                .as_ref()
                .expect("checked match position")
                .clone(),
            // NO_CLONE: ReplaceFindRequest needs its own captured identity after this
            // render/dispatch; keep the original catalog, input or owner available
            // for the later checks in this operation.
            replacement: session.replacement.text.clone(),
        })
    }

    pub fn can_replace_find(&self, request: &ReplaceFindRequest) -> bool {
        (self.replace_available || self.mock_preview)
            && self.can_open_find()
            && request
                .replacement
                .chars()
                .nth(MAX_EDITOR_SCALARS)
                .is_none()
            && self.current_replace_find_request().as_ref() == Some(request)
            && self.find.as_ref().is_some_and(|session| {
                session.pending.is_none() && session.replacement.pending.is_none()
            })
            && self.can_edit_cell(&edit_target(request))
    }

    pub fn begin_replace_find(&mut self, request_id: u64, request: ReplaceFindRequest) -> bool {
        if request_id == 0 || !self.can_replace_find(&request) {
            return false;
        }
        let filter = self.find_filter_witness();
        let session = self.find.as_mut().expect("admitted match receipt");
        session.error = None;
        session.replacement.pending = Some(Pending {
            request_id,
            request,
            publication: self.focus_publication,
            filter,
        });
        true
    }

    pub fn accepts_replace_find_reply(&self, request_id: u64) -> bool {
        !self.edit_dialog_open()
            && !self.zoom_dialog_open()
            && !self.table_filter_pending()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && self.found_receipt_current()
            && self.find.as_ref().is_some_and(|session| {
                session.replacement.pending.as_ref().is_some_and(|pending| {
                    pending.request_id == request_id
                        && pending.publication == self.focus_publication
                        && pending.filter == self.find_filter_witness()
                        && self.current_replace_find_request().as_ref() == Some(&pending.request)
                })
            })
    }

    pub fn set_replace_find_draft(&mut self, request_id: u64, draft: EditDraft) -> bool {
        if !self.accepts_replace_find_reply(request_id) {
            return false;
        }
        let pending = self
            .find
            .as_ref()
            .expect("accepted replacement")
            .replacement
            .pending
            .as_ref()
            .expect("accepted replacement request");
        if draft.target != edit_target(&pending.request)
            || draft.initial.is_null
            || !self.can_edit_cell(&draft.target)
        {
            self.fail_replace_find(request_id, self.localizer.text("find-invalid-result"));
            return false;
        }
        // The runtime's complete candidate enters the ordinary editor unchanged.
        // No review, persistence, or NULL inference occurs at this transition.
        self.find = None;
        // NO_CLONE: the retained command or exact-owner witness needs its own captured identity
        // after this render/dispatch; keep the original catalog, input or owner
        // available for the later checks in this operation.
        self.begin_edit(draft.target.clone(), request_id);
        self.set_edit_draft(request_id, draft);
        matches!(self.edit, Some(EditDialog::Draft { .. }))
    }

    pub fn fail_replace_find(&mut self, request_id: u64, message: String) {
        if self.accepts_replace_find_reply(request_id) {
            let session = self.find.as_mut().expect("current replacement request");
            session.replacement.pending = None;
            session.error = Some(message);
        }
    }

    pub(super) fn replace_request_id(&self) -> Option<u64> {
        Some(self.find.as_ref()?.replacement.pending.as_ref()?.request_id)
    }
}

#[cfg(test)]
mod tests;
