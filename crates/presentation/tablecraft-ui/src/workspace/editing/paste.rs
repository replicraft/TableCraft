//! Literal clipboard text enters the existing reviewed single-cell edit workflow.
use super::{EditSeed, EditTarget};
use crate::workspace::{Action, DocumentView, State};
use std::mem;

pub(in crate::workspace) struct ClipboardRequest {
    target: EditTarget,
    publication: u64,
    focus: Option<egui::Id>,
    cancelled: bool,
}

impl State {
    pub fn can_paste_cell(&self, target: &EditTarget) -> bool {
        !self.busy
            && !self.edit_dialog_open()
            && !self.parameter_prompt_open()
            && !self.find_window_open()
            && !self.table_filter_pending()
            && self.can_edit_cell(target)
            && self.current_paste_target_unchecked().as_ref() == Some(target)
    }

    fn current_paste_target_unchecked(&self) -> Option<EditTarget> {
        if self.view != DocumentView::Table || self.table_selection.row_selected() {
            return None;
        }
        let (row, column) = self.table_selection.position()?;
        self.data.as_ref()?.edit_target(row, column)
    }

    pub(in crate::workspace) fn current_paste_target(&self) -> Option<EditTarget> {
        if self.paste_request.is_some() || self.ribbon_paste.is_some() {
            return None;
        }
        let target = self.current_paste_target_unchecked()?;
        self.can_paste_cell(&target).then_some(target)
    }

    pub fn begin_paste_edit(&mut self, target: EditTarget, text: String, request_id: u64) -> bool {
        self.begin_seeded_edit(target, EditSeed::Text(text), request_id)
    }

    pub(in crate::workspace) fn request_paste(&mut self, context: &egui::Context) {
        let Some(target) = self.current_paste_target() else {
            return;
        };
        self.paste_request = Some(ClipboardRequest {
            target,
            publication: self.focus_publication,
            focus: context.memory(|memory| memory.focused()),
            cancelled: false,
        });
        context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }

    /// RequestPaste has no reply identifier. Keep an invalidated request as a
    /// tombstone until its next Paste event arrives, before any text widget draws.
    pub(in crate::workspace) fn receive_requested_paste(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) -> Option<ClipboardRequest> {
        let mut request = self.paste_request.take()?;
        request.cancelled |= !self.clipboard_request_current(&request, context);
        let mut pasted = None;
        let mut count = 0;
        context.input_mut(|input| {
            let events = mem::take(&mut input.events);
            for event in events {
                if let egui::Event::Paste(text) = event {
                    count += 1;
                    if count == 1 {
                        pasted = Some(text);
                    }
                } else {
                    input.events.push(event);
                }
            }
        });
        if count > 1 {
            self.error = Some(self.localizer.text("editing-paste-multiple"));
            return None;
        }
        match pasted {
            Some(_) if request.cancelled => {
                self.error = Some(self.localizer.text("editing-paste-stale"));
                None
            }
            Some(text) => {
                actions.push(Action::PasteCell {
                    // NO_CLONE: Action::PasteCell needs its own captured identity after this
                    // render/dispatch; keep the original catalog, input or owner
                    // available for the later checks in this operation.
                    target: request.target.clone(),
                    text,
                });
                Some(request)
            }
            None => {
                self.paste_request = Some(request);
                None
            }
        }
    }

    fn clipboard_request_current(
        &self,
        request: &ClipboardRequest,
        context: &egui::Context,
    ) -> bool {
        request.publication == self.focus_publication
            && request.focus == context.memory(|memory| memory.focused())
            && self.can_paste_cell(&request.target)
    }

    pub(in crate::workspace) fn finish_paste(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
        receipt: Option<ClipboardRequest>,
    ) {
        let receipt_current = receipt
            .as_ref()
            .is_none_or(|request| self.clipboard_request_current(request, context));
        if !receipt_current {
            self.error = Some(self.localizer.text("editing-paste-stale"));
        }
        if let Some(mut request) = self.paste_request.take() {
            request.cancelled |= request.publication != self.focus_publication
                || request.focus != context.memory(|memory| memory.focused())
                || !self.can_paste_cell(&request.target);
            self.paste_request = Some(request);
        }
        actions.retain(|action| {
            let Action::PasteCell { target, .. } = action else {
                return true;
            };
            receipt_current && self.paste_request.is_none() && self.can_paste_cell(target)
        });
    }
}
