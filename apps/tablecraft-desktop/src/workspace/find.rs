//! Search the complete native source, then retrieve its checked result page.
use super::filtering::filtered_publication;
use super::{Desktop, PendingOperation};
use tablecraft_runtime::{
    self as runtime, Command, FilteredData, FindPosition as RuntimePosition,
    FindRequest as RuntimeRequest,
};
use tablecraft_ui::{self as ui, FilterTarget, FindPosition, FindRequest, FindSpan};

#[derive(Clone)]
pub(super) struct FilterWitness {
    pub(super) target: FilterTarget,
    pub(super) enabled: bool,
}

impl Desktop {
    pub(super) fn find_next(&mut self, request: FindRequest) {
        if self.editing.has_save() || self.state.busy || !self.state.can_find_next(&request) {
            return;
        }

        let filter = self
            .state
            .active_filter_target()
            .map(|target| FilterWitness {
                target,
                enabled: self.state.table_filter_enabled(),
            });
        let command = match &filter {
            Some(filter) => {
                let Some(handle) = self.filter_handle(&filter.target) else {
                    self.state.error = Some(self.state.localizer.text("filtering-stale"));
                    return;
                };
                Command::FindFiltered {
                    handle,
                    request: runtime_request(request.clone()),
                    enabled: filter.enabled,
                }
            }
            None => Command::FindNext {
                request: runtime_request(request.clone()),
            },
        };
        let request_id = self.worker.request(command);
        self.generation = request_id;
        self.state.begin_find(request_id, request.clone());
        self.pending = PendingOperation::Find {
            request_id,
            request,
            filter,
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
            filter,
        } = &self.pending
        else {
            return;
        };
        if generation != *request_id
            || !runtime_request_matches(pending, &request)
            || !self.state.accepts_find_reply(*request_id, pending)
        {
            return;
        }

        let request_id = *request_id;
        let request = pending.clone();
        let filter = filter.clone();
        let Some(position) = position.map(ui_position) else {
            if filter.is_some() {
                self.state
                    .set_filtered_find_result(request_id, request, None, None);
            } else {
                self.state.set_find_result(request_id, request, None, None);
            }
            self.pending = PendingOperation::Read;
            return;
        };
        // Page publication has its own native allowance. Neither a failed page
        // nor cancellation may publish the match without its original revision.
        let command = match &filter {
            Some(filter) => {
                let Some(handle) = self.filter_handle(&filter.target) else {
                    self.state
                        .fail_find(request_id, self.state.localizer.text("filtering-stale"));
                    return;
                };
                let Some(offset) =
                    handle.display_offset_for_ordinal(filter.enabled, position.parent_ordinal)
                else {
                    self.state.fail_find(
                        request_id,
                        self.state.localizer.text("filtering-invalid-result"),
                    );
                    return;
                };
                Command::FilterPage {
                    handle,
                    offset,
                    enabled: filter.enabled,
                }
            }
            None => Command::Page {
                table: request.target.table.clone(),
                offset: position.parent_ordinal,
            },
        };
        self.generation = self.worker.request(command);
        self.pending = PendingOperation::FindPage {
            request_id,
            request,
            position,
            filter,
        };
        self.state.busy = true;
    }

    pub(super) fn publish_filtered_found(&mut self, generation: u64, result: FilteredData) {
        let PendingOperation::FindPage {
            request_id,
            request,
            position,
            filter: Some(filter),
        } = &self.pending
        else {
            return;
        };
        if generation != self.generation || !self.state.accepts_find_reply(*request_id, request) {
            return;
        }
        let request_id = *request_id;
        let request = request.clone();
        let position = position.clone();
        let valid = result.target.filter_id == filter.target.filter_id
            && result.target.document_revision == filter.target.document_revision
            && result.target.table == filter.target.table
            && result.enabled == filter.enabled;
        if !valid {
            self.state.fail_find(
                request_id,
                self.state.localizer.text("filtering-invalid-result"),
            );
            return;
        }
        match filtered_publication(result) {
            Ok((handle, publication)) => {
                if self.state.set_filtered_find_result(
                    request_id,
                    request,
                    Some(position),
                    Some(publication),
                ) {
                    self.filter = Some(handle);
                }
            }
            Err(_) => self.state.fail_find(
                request_id,
                self.state.localizer.text("filtering-invalid-result"),
            ),
        }
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
            PendingOperation::ReplaceFind {
                request_id: current,
            } => *current == request_id,
            _ => false,
        };
        if active {
            self.worker.cancel();
            self.state.busy = false;
            let message = self.state.localizer.text("shell-operation-cancelled");
            if matches!(self.pending, PendingOperation::ReplaceFind { .. }) {
                self.state.fail_replace_find(request_id, message);
            } else {
                self.state.fail_find(request_id, message);
            }
            self.pending = PendingOperation::Read;
        }
    }
}

/// Compare every mapped request field without copying the retained UI request.
fn runtime_request_matches(pending: &FindRequest, reply: &RuntimeRequest) -> bool {
    let scope_matches = match (&pending.scope, &reply.scope) {
        (ui::FindScope::Field(left), runtime::FindScope::Field(right)) => left == right,
        (ui::FindScope::Table, runtime::FindScope::Table) => true,
        _ => false,
    };
    let matching_matches = matches!(
        (pending.matching, reply.matching),
        (ui::FindMatch::Any, runtime::FindMatch::Any)
            | (ui::FindMatch::Whole, runtime::FindMatch::Whole)
            | (ui::FindMatch::Start, runtime::FindMatch::Start)
    );
    let direction_matches = matches!(
        (pending.direction, reply.direction),
        (ui::FindDirection::All, runtime::FindDirection::All)
            | (ui::FindDirection::Up, runtime::FindDirection::Up)
            | (ui::FindDirection::Down, runtime::FindDirection::Down)
    );
    let start_matches = match (&pending.start, &reply.start) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.parent_ordinal == right.parent_ordinal
                && left.column == right.column
                && match (&left.span, &right.span) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        left.start_utf16 == right.start_utf16 && left.end_utf16 == right.end_utf16
                    }
                    _ => false,
                }
        }
        _ => false,
    };

    pending.target.document_revision == reply.target.document_revision
        && pending.target.table == reply.target.table
        && scope_matches
        && pending.pattern == reply.pattern
        && matching_matches
        && direction_matches
        && start_matches
        && pending.match_case == reply.match_case
        && pending.formatted == reply.formatted
        && pending.format_options == reply.format_options
}

pub(super) fn runtime_request(request: FindRequest) -> RuntimeRequest {
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
