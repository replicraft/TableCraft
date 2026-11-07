//! Retain first-record input across schema publication and actual native binding.
use super::{Action, InitialRecord, InsertInput, InsertTarget, ObjectKind, ObjectTarget, State};
use crate::localization::tr;
use egui::{Context, Id, Modal};

const MAX_FAILURE_BYTES: usize = 4096;
const MODAL_OUTER_MARGIN: f32 = 48.0;
const MODAL_MIN_WIDTH: f32 = 240.0;
const MODAL_MAX_WIDTH: f32 = 600.0;

pub(super) struct Continuation {
    target: InsertTarget,
    record: InitialRecord,
    request: Option<u64>,
    auto_start: bool,
    error: bool,
}

impl State {
    /// Prepare before discarding any schema editor. Failure returns all input.
    pub(super) fn prepare_initial_record(
        &self,
        target: &ObjectTarget,
        record: InitialRecord,
    ) -> Result<Continuation, InitialRecord> {
        let ObjectKind::Table { name } = &target.kind else {
            return Err(record);
        };
        if self.initial_record.is_some()
            || name != record.snapshot.table_name()
            || record.inputs.len() != record.snapshot.fields().len()
            || !matches!(record.inputs.first(), Some(InsertInput::Omitted))
        {
            return Err(record);
        }
        Ok(Continuation {
            target: InsertTarget {
                document_revision: target.document_revision,
                table: name.clone(),
            },
            record,
            request: None,
            auto_start: true,
            error: false,
        })
    }

    pub fn initial_record_pending(&self) -> bool {
        self.initial_record.is_some()
    }

    pub(super) fn initial_record_insert_started(&mut self, request_id: u64) {
        let Some(continuation) = &mut self.initial_record else {
            return;
        };
        if let Some(super::insertion::Dialog::Checking { target, .. }) = &self.insertion
            && *target == continuation.target
        {
            continuation.request = Some(request_id);
            continuation.auto_start = false;
        }
    }

    pub(super) fn adopt_initial_record(&mut self, request_id: u64, draft_id: u64) {
        let Some(mut continuation) = self.initial_record.take() else {
            return;
        };
        if continuation.request != Some(request_id) {
            self.initial_record = Some(continuation);
            return;
        }
        if self.seed_initial_insert(draft_id, &continuation.record) {
            // The real insertion editor now owns an exact copy, under the
            // runtime-issued draft and fresh revision. No original is lost.
            self.initial_record_notice = Some(continuation.target);
        } else {
            continuation.request = None;
            continuation.auto_start = false;
            continuation.error = true;
            self.insertion = None;
            self.initial_record = Some(continuation);
        }
    }

    pub(super) fn reject_initial_record_draft(&mut self, request_id: u64) {
        if let Some(continuation) = &mut self.initial_record
            && continuation.request == Some(request_id)
        {
            continuation.request = None;
            continuation.auto_start = false;
            continuation.error = true;
            self.insertion = None;
        }
    }

    pub(super) fn initial_record_insert_closed(&mut self) {
        self.initial_record_notice = None;
        if let Some(continuation) = &mut self.initial_record {
            continuation.request = None;
            continuation.auto_start = false;
            continuation.error = true;
        }
    }

    pub(super) fn initial_record_dialog(&mut self, context: &Context, actions: &mut Vec<Action>) {
        if self.insertion.is_some() {
            return;
        }
        let Some(continuation) = &self.initial_record else {
            return;
        };
        let target = continuation.target.clone();
        let ready = self.can_insert_row(&target);
        if continuation.auto_start && ready {
            self.initial_record.as_mut().unwrap().auto_start = false;
            actions.push(Action::BeginInsert { target });
            return;
        }
        let busy = self.busy;
        let bound = self.database.as_ref().is_some_and(|database| {
            database.document_revision == target.document_revision
                && database
                    .tables
                    .iter()
                    .any(|table| table.name == target.table && !table.system)
        });
        let failed = continuation.error || !bound;
        let mut retry = false;
        let mut discard = false;
        Modal::new(Id::new("initial_record_continuation")).show(context, |ui| {
            ui.set_width(
                (context.content_rect().width() - MODAL_OUTER_MARGIN)
                    .clamp(MODAL_MIN_WIDTH, MODAL_MAX_WIDTH),
            );
            ui.heading(tr(ui, "insertion-title"));
            ui.label(tr(ui, "table-draft-record-schema-saved"));
            if busy {
                ui.spinner();
                ui.label(tr(ui, "table-record-preparing"));
            } else {
                if failed {
                    ui.colored_label(ui.visuals().error_fg_color, tr(ui, "table-record-invalid"));
                }
                if let Some(error) = &self.error {
                    // Existing host diagnostics are bounded separately. Do not
                    // copy row contents into this publication failure message.
                    let mut end = error.len().min(MAX_FAILURE_BYTES);
                    while !error.is_char_boundary(end) {
                        end -= 1;
                    }
                    ui.label(&error[..end]);
                }
            }
            ui.horizontal(|ui| {
                retry = ui
                    .add_enabled(
                        !busy && bound,
                        egui::Button::new(tr(ui, "table-record-retry")),
                    )
                    .clicked();
                discard = ui.button(tr(ui, "table-record-discard")).clicked();
            });
        });
        if discard {
            // This abandons only the unsaved record, never the committed table.
            self.initial_record = None;
        } else if retry {
            let continuation = self.initial_record.as_mut().unwrap();
            continuation.auto_start = true;
            continuation.error = false;
            if !ready {
                actions.push(Action::Page {
                    table: target.table,
                    offset: 0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
