//! Search the complete native source, then retrieve its checked result page.
use super::{Desktop, PendingOperation};
use tablecraft_runtime::{
    self as runtime, Command, FindPosition as RuntimePosition, FindRequest as RuntimeRequest,
};
use tablecraft_ui::{self as ui, FindPosition, FindRequest, FindSpan};

impl Desktop {
    pub(super) fn find_next(&mut self, request: FindRequest) {
        if self.editing.has_save() || self.state.busy || !self.state.can_find_next(&request) {
            return;
        }

        let command = Command::FindNext {
            request: runtime_request(request.clone()),
        };
        let request_id = self.worker.request(command);
        self.generation = request_id;
        self.state.begin_find(request_id, request.clone());
        self.pending = PendingOperation::Find {
            request_id,
            request,
        };
        self.state.busy = true;
    }

    pub(super) fn publish_found(
        &mut self,
        generation: u64,
        request: RuntimeRequest,
        position: Option<RuntimePosition>,
    ) {
        let PendingOperation::Find {
            request_id,
            request: pending,
        } = &self.pending
        else {
            return;
        };
        if generation != *request_id
            || runtime_request(pending.clone()) != request
            || !self.state.accepts_find_reply(*request_id, pending)
        {
            return;
        }

        let request_id = *request_id;
        let request = pending.clone();
        let Some(position) = position.map(ui_position) else {
            self.state.set_find_result(request_id, request, None, None);
            self.pending = PendingOperation::Read;
            return;
        };
        // Page publication has its own native allowance. Neither a failed page
        // nor cancellation may publish the match without its original revision.
        self.generation = self.worker.request(Command::Page {
            table: request.target.table.clone(),
            offset: position.parent_ordinal,
        });
        self.pending = PendingOperation::FindPage {
            request_id,
            request,
            position,
        };
        self.state.busy = true;
    }

    pub(super) fn cancel_find(&mut self, request_id: u64) {
        let active = match &self.pending {
            PendingOperation::Find {
                request_id: current,
                ..
            }
            | PendingOperation::FindPage {
                request_id: current,
                ..
            } => *current == request_id,
            _ => false,
        };
        if active {
            self.worker.cancel();
            self.state.busy = false;
            self.state.fail_find(
                request_id,
                self.state.localizer.text("shell-operation-cancelled"),
            );
            self.pending = PendingOperation::Read;
        }
    }
}

fn runtime_request(request: FindRequest) -> RuntimeRequest {
    RuntimeRequest {
        target: runtime::FindTarget {
            document_revision: request.target.document_revision,
            table: request.target.table,
        },
        scope: match request.scope {
            ui::FindScope::Field(column) => runtime::FindScope::Field(column),
            ui::FindScope::Table => runtime::FindScope::Table,
        },
        pattern: request.pattern,
        matching: match request.matching {
            ui::FindMatch::Any => runtime::FindMatch::Any,
            ui::FindMatch::Whole => runtime::FindMatch::Whole,
            ui::FindMatch::Start => runtime::FindMatch::Start,
        },
        direction: match request.direction {
            ui::FindDirection::All => runtime::FindDirection::All,
            ui::FindDirection::Up => runtime::FindDirection::Up,
            ui::FindDirection::Down => runtime::FindDirection::Down,
        },
        start: request.start.map(|position| runtime::FindPosition {
            parent_ordinal: position.parent_ordinal,
            column: position.column,
            span: position.span.map(|span| runtime::FindSpan {
                start_utf16: span.start_utf16,
                end_utf16: span.end_utf16,
            }),
        }),
        match_case: request.match_case,
        formatted: request.formatted,
        format_options: request.format_options,
    }
}

fn ui_position(position: RuntimePosition) -> FindPosition {
    FindPosition {
        parent_ordinal: position.parent_ordinal,
        column: position.column,
        span: position.span.map(|span| FindSpan {
            start_utf16: span.start_utf16,
            end_utf16: span.end_utf16,
        }),
    }
}

#[cfg(test)]
mod tests;
