//! Closed-object rename presentation; native admission and persistence stay in hosts.
use super::{ObjectKind, ObjectTarget};
use crate::localization::tr;
use crate::workspace::{Action, EditSaveOutcome, State, diagnostics, save_display};
use egui::{
    Id, Key, TextEdit,
    text::{CCursor, CCursorRange},
};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_NAME_CHARACTERS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_CHARACTERS * char::MAX.len_utf8();
const MAX_REPORT_BYTES: usize = 4096;
const DIALOG_WIDTH: f32 = 340.0;
const VIEWPORT_MARGIN: f32 = 40.0;
const MIN_DIALOG_WIDTH: f32 = 100.0;
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(0);

/// Exact user-issued naming request; display facts do not grant native authority.
#[derive(Clone)]
pub struct TableRenameRequest {
    /// Unique presentation request used to reject stale callbacks.
    pub id: u64,
    /// Open document revision that owns the original table.
    pub document_revision: u64,
    /// Exact original catalog spelling selected in the Navigation Pane.
    pub table: String,
    /// Requested catalog spelling, without implicit trimming or normalization.
    pub new_name: String,
}

/// Presentation owner retained across naming, review, save and terminal reporting.
pub(in crate::workspace) struct Rename {
    target: ObjectTarget,
    name: String,
    focus: bool,
    error: Option<String>,
    phase: Phase,
}

enum Phase {
    Editing,
    Reviewing(TableRenameRequest),
    Saving {
        id: u64,
        cancelled: bool,
        resolving: bool,
    },
    Finished {
        id: u64,
        outcome: EditSaveOutcome,
    },
}

impl State {
    /// Admit naming UI only for a closed, local user table on a supporting host.
    /// Native format, name and dependency checks remain the runtime's responsibility.
    pub fn can_rename_table(&self, target: &ObjectTarget) -> bool {
        if self.native_mutation_blocked()
            || !self.table_rename_available
            || !self.object_switch_ready()
            || !self.object_tabs().is_empty()
        {
            return false;
        }
        let Some(database) = &self.database else {
            return false;
        };
        let ObjectKind::Table { name } = &target.kind else {
            return false;
        };

        database.document_revision == target.document_revision
            && database.tables.iter().any(|table| {
                table.name == *name && table.local && !table.system && name.len() <= MAX_NAME_BYTES
            })
    }

    /// Open the naming editor while retaining the selected table and document revision.
    pub fn begin_table_rename(&mut self, target: ObjectTarget) -> bool {
        if !self.can_rename_table(&target) {
            return false;
        }
        let ObjectKind::Table { name } = &target.kind else {
            return false;
        };
        self.table_rename = Some(Rename {
            name: name.clone(),
            target,
            focus: true,
            error: None,
            phase: Phase::Editing,
        });
        true
    }

    /// Whether naming, review, saving or a retained terminal outcome owns the dialog.
    pub fn table_rename_open(&self) -> bool {
        self.table_rename.is_some()
    }

    /// Check that a callback still belongs to the exact issued naming request.
    pub fn table_rename_request_current(&self, request: &TableRenameRequest) -> bool {
        if self.native_mutation_blocked() {
            return false;
        }
        let current_document = self
            .database
            .as_ref()
            .is_some_and(|database| database.document_revision == request.document_revision);
        if !current_document {
            return false;
        }
        let Some(rename) = &self.table_rename else {
            return false;
        };
        let Phase::Reviewing(current) = &rename.phase else {
            return false;
        };

        current.id == request.id
            && current.table == request.table
            && current.new_name == request.new_name
            && current.document_revision == request.document_revision
    }

    /// Return a matching, unaccepted review to editable naming with a bounded diagnosis.
    pub fn fail_table_rename(&mut self, request: u64, message: String) {
        if let Some(rename) = &mut self.table_rename
            && matches!(&rename.phase, Phase::Reviewing(current) if current.id == request)
        {
            rename.error = Some(if message.capacity() <= MAX_REPORT_BYTES {
                message
            } else {
                self.localizer.text("objects-memory-limit")
            });
            rename.phase = Phase::Editing;
            rename.focus = true;
        }
    }

    /// Drop only the matching review before a save capability has been accepted.
    pub fn cancel_table_rename_review(&mut self, request_id: u64) -> bool {
        let current = self.table_rename.as_ref().is_some_and(
            |rename| matches!(&rename.phase, Phase::Reviewing(request) if request.id == request_id),
        );
        if current {
            self.table_rename = None;
        }
        current
    }

    /// Reselect a freshly reopened catalog entry without opening its table.
    pub fn select_renamed_table(&mut self, target: ObjectTarget) -> bool {
        self.synchronize_objects();
        self.select_navigation_object(target)
    }

    /// Dismiss a matching review whose native operation confirmed unchanged spelling.
    pub fn finish_unchanged_table_rename(&mut self, request: &TableRenameRequest) -> bool {
        if !self.table_rename_request_current(request) {
            return false;
        }
        self.table_rename = None;
        true
    }

    /// Transfer the exact issued request to a nonreplaceable save operation.
    pub fn begin_table_rename_save(&mut self, request: &TableRenameRequest, id: u64) -> bool {
        if !self.table_rename_request_current(request) {
            return false;
        }
        self.table_rename.as_mut().unwrap().phase = Phase::Saving {
            id,
            cancelled: false,
            resolving: false,
        };
        true
    }

    /// Publish a matching save outcome without discarding retained resource permissions.
    pub fn set_table_rename_save_outcome(&mut self, id: u64, mut outcome: EditSaveOutcome) {
        let message = match &mut outcome {
            EditSaveOutcome::Saved(message)
            | EditSaveOutcome::Cancelled(message)
            | EditSaveOutcome::Failed(message)
            | EditSaveOutcome::Retained { message, .. } => message,
        };
        if message.capacity() > MAX_REPORT_BYTES {
            *message = self.localizer.text("objects-memory-limit");
        }

        if let Some(rename) = &mut self.table_rename
            && matches!(rename.phase, Phase::Saving { id: current, .. } if current == id)
        {
            rename.phase = Phase::Finished { id, outcome };
        }
    }

    /// Expose only the recovery or release permission reported by this save owner.
    pub fn can_resolve_table_rename_save(&self, id: u64, recover: bool) -> bool {
        let Some(rename) = &self.table_rename else {
            return false;
        };
        let Phase::Finished {
            id: current,
            outcome:
                EditSaveOutcome::Retained {
                    can_recover,
                    can_release,
                    ..
                },
        } = &rename.phase
        else {
            return false;
        };

        *current == id && if recover { *can_recover } else { *can_release }
    }

    /// Transfer a terminal save owner to its explicitly started recovery or release.
    pub fn begin_table_rename_resolution(&mut self, id: u64, next: u64) {
        if let Some(rename) = &mut self.table_rename
            && matches!(rename.phase, Phase::Finished { id: current, .. } if current == id)
        {
            rename.phase = Phase::Saving {
                id: next,
                cancelled: false,
                resolving: true,
            };
        }
    }

    /// Permit dismissal only for this save's terminal, resource-free outcome.
    pub fn can_acknowledge_table_rename_save(&self, id: u64) -> bool {
        let Some(rename) = &self.table_rename else {
            return false;
        };
        let Phase::Finished {
            id: current,
            outcome,
        } = &rename.phase
        else {
            return false;
        };

        *current == id && !matches!(outcome, EditSaveOutcome::Retained { .. })
    }

    /// Dismiss only an acknowledged terminal outcome that retains no save resources.
    pub fn acknowledge_table_rename_save(&mut self, id: u64) -> bool {
        if !self.can_acknowledge_table_rename_save(id) {
            return false;
        }
        self.table_rename = None;
        true
    }

    /// Render naming or the existing retained-save status without native dispatch.
    pub(in crate::workspace) fn table_rename_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(rename) = &mut self.table_rename else {
            return;
        };
        let mut close = false;
        let mut submit = false;
        let response = egui::Modal::new(Id::new("table_rename")).show(context, |ui| {
            ui.set_width(
                DIALOG_WIDTH
                    .min((context.content_rect().width() - VIEWPORT_MARGIN).max(MIN_DIALOG_WIDTH)),
            );
            ui.strong(tr(ui, "navigation-rename-table"));
            ui.separator();
            match &mut rename.phase {
                Phase::Editing => {
                    let label = ui.label(tr(ui, "navigation-rename-name"));
                    let mut field = TextEdit::singleline(&mut rename.name)
                        .char_limit(MAX_NAME_CHARACTERS)
                        .desired_width(ui.available_width())
                        .show(ui);
                    field.response.response = field.response.response.labelled_by(label.id);
                    if rename.focus {
                        field.response.request_focus();
                        field.state.cursor.set_char_range(Some(CCursorRange::two(
                            CCursor::new(0),
                            CCursor::new(rename.name.chars().count()),
                        )));
                        field.state.store(ui.ctx(), field.response.id);
                        rename.focus = false;
                    }
                    if let Some(error) = &rename.error {
                        diagnostics::show(ui, ui.id().with("rename_error"), error);
                    }
                    ui.horizontal(|ui| {
                        submit = ui
                            .add_enabled(
                                !rename.name.is_empty(),
                                egui::Button::new(tr(ui, "navigation-rename")),
                            )
                            .clicked();
                        close = ui.button(tr(ui, "shell-cancel")).clicked();
                    });
                    submit |= ui
                        .input(|input| input.key_pressed(Key::Enter) && input.modifiers.is_none())
                        && !rename.name.is_empty();
                }
                Phase::Reviewing(request) => {
                    ui.spinner();
                    ui.label(tr(ui, "navigation-rename-reviewing"));
                    if ui.button(tr(ui, "shell-cancel")).clicked() {
                        actions.push(Action::CancelTableRenameReview {
                            request_id: request.id,
                        });
                    }
                }
                Phase::Saving {
                    id,
                    cancelled,
                    resolving,
                } => {
                    save_display::pending(ui, *cancelled, *resolving);
                    if !*cancelled && ui.button(tr(ui, "editing-request-cancellation")).clicked() {
                        *cancelled = true;
                        actions.push(Action::CancelEditSave { save_id: *id });
                    }
                }
                Phase::Finished { id, outcome } => {
                    save_display::outcome(ui, outcome);
                    if let EditSaveOutcome::Retained {
                        can_recover,
                        can_release,
                        ..
                    } = outcome
                    {
                        if ui
                            .add_enabled(*can_recover, egui::Button::new(tr(ui, "editing-recover")))
                            .clicked()
                        {
                            actions.push(Action::RecoverEditSave { save_id: *id });
                        }
                        if ui
                            .add_enabled(*can_release, egui::Button::new(tr(ui, "editing-release")))
                            .clicked()
                        {
                            actions.push(Action::ReleaseEditSave { save_id: *id });
                        }
                    } else if ui.button(tr(ui, "backstage-close")).clicked() {
                        actions.push(Action::AcknowledgeEditSave { save_id: *id });
                    }
                }
            }
        });
        if response.should_close() {
            match &mut rename.phase {
                Phase::Editing => close = true,
                Phase::Reviewing(request) => {
                    actions.push(Action::CancelTableRenameReview {
                        request_id: request.id,
                    });
                }
                Phase::Saving { id, cancelled, .. } if !*cancelled => {
                    *cancelled = true;
                    actions.push(Action::CancelEditSave { save_id: *id });
                }
                _ => {}
            }
        }
        if submit {
            if let Ok(previous) =
                NEXT_REQUEST
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            {
                let ObjectKind::Table { name } = &rename.target.kind else {
                    return;
                };
                let request = TableRenameRequest {
                    id: previous + 1,
                    document_revision: rename.target.document_revision,
                    table: name.clone(),
                    new_name: rename.name.clone(),
                };
                rename.phase = Phase::Reviewing(request.clone());
                actions.push(Action::RenameTable { request });
            } else {
                rename.error = Some(self.localizer.text("objects-memory-limit"));
            }
        }
        if close {
            self.table_rename = None;
        }
    }
}

#[cfg(test)]
mod tests;
