//! Inert File/New draft and source-bound host publication requests.
use super::{Action, State, backstage::Backstage, diagnostics, ribbon::Icon};
use crate::{localization::tr, theme::palette};
use egui::{Frame, Margin, TextEdit, Ui};

const MAX_DESTINATION_BYTES: usize = 4096;

/// Explicit user request; the host validates the native destination and format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreationRequest {
    pub id: u64,
    pub destination: String,
}

/// Publication facts, without implying native Access save qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationOutcome {
    Created,
    NotPublished,
    VisibilityUncertain,
    PublishedUnconfirmed,
    PublishedCleanupFailure,
    Unsupported,
}

pub(super) struct Creation {
    destination: String,
    selected: bool,
    open: bool,
    sequence: u64,
    pending: Option<u64>,
    issued: Option<u64>,
    cancelling: bool,
    outcome: Option<CreationOutcome>,
    path_too_long: bool,
}

impl Default for Creation {
    fn default() -> Self {
        Self {
            destination: "Database1.accdb".into(),
            selected: false,
            open: false,
            sequence: 0,
            pending: None,
            issued: None,
            cancelling: false,
            outcome: None,
            path_too_long: false,
        }
    }
}

impl State {
    pub fn creation_destination(&self) -> &str {
        &self.creation.destination
    }

    /// Install a host-selected destination. Filesystem identity stays with host.
    pub fn set_creation_destination(&mut self, destination: String) -> bool {
        if self.busy || self.creation.pending.is_some() || destination.len() > MAX_DESTINATION_BYTES
        {
            return false;
        }
        self.creation.issued = None;
        self.creation.path_too_long = false;
        self.creation.selected = !destination.is_empty();
        self.creation.destination = destination;
        self.creation.outcome = None;
        self.error = None;
        true
    }

    pub fn creation_is_pending(&self) -> bool {
        self.creation.pending.is_some()
    }

    pub fn request_database_creation(&mut self) -> Option<CreationRequest> {
        if !self.creation_ready() {
            return None;
        }
        self.creation.sequence = self.creation.sequence.checked_add(1)?;
        self.creation.issued = Some(self.creation.sequence);
        Some(CreationRequest {
            id: self.creation.sequence,
            destination: self.creation.destination.clone(),
        })
    }

    /// Recheck the exact draft captured by the explicit Create action.
    pub fn begin_database_creation(&mut self, request: &CreationRequest) -> bool {
        if !self.creation_ready()
            || request.id == 0
            || self.creation.issued != Some(request.id)
            || request.destination != self.creation.destination
        {
            return false;
        }
        self.creation.issued = None;
        self.creation.pending = Some(request.id);
        self.creation.outcome = None;
        self.creation.cancelling = false;
        self.busy = true;
        true
    }

    pub fn mark_creation_cancelling(&mut self, id: u64) -> bool {
        if self.creation.pending != Some(id) {
            return false;
        }
        self.creation.cancelling = true;
        true
    }

    pub fn finish_database_creation(&mut self, id: u64, outcome: CreationOutcome) -> bool {
        if self.creation.pending != Some(id) {
            return false;
        }
        self.creation.pending = None;
        self.creation.cancelling = false;
        self.creation.outcome = Some(outcome);
        self.busy = false;
        // An uncertain or published outcome must never invite an accidental retry.
        if outcome != CreationOutcome::NotPublished {
            self.creation.selected = false;
        }
        true
    }

    pub(super) fn open_creation(&mut self) {
        if self.creation_available
            && !self.busy
            && !self.insertion_dialog_open()
            && !self.table_draft_field_delete_open()
        {
            self.creation.open = true;
            self.creation.outcome = None;
            self.error = None;
            self.backstage = Backstage::New;
        }
    }

    pub(super) fn creation_form_open(&self) -> bool {
        self.creation.open
    }

    fn creation_ready(&self) -> bool {
        self.creation_available
            && !self.table_draft_field_delete_open()
            && self.creation.open
            && self.creation.selected
            && !self.busy
            && self.creation.pending.is_none()
            && self.creation.destination.len() <= MAX_DESTINATION_BYTES
            && self
                .creation
                .destination
                .to_ascii_lowercase()
                .ends_with(".accdb")
    }

    pub(super) fn creation_pane(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        ui.heading(tr(ui, "creation-title"));
        ui.add_space(16.0);
        let colors = palette(ui.ctx());
        Frame::new()
            .fill(colors.title_fill)
            .stroke(egui::Stroke::new(1.0, colors.grid_line))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_max_width(440.0);
                ui.horizontal(|ui| {
                    ui.add(Icon::DatabaseFile.image(ui, 48.0));
                    ui.vertical(|ui| {
                        ui.strong(tr(ui, "creation-title"));
                        ui.label(tr(ui, "creation-format"));
                    });
                });
                ui.add_space(12.0);
                ui.label(tr(ui, "creation-empty-note"));
                ui.add_space(16.0);
                let name_label = tr(ui, "creation-file-name");
                ui.label(&name_label);
                let input_id = ui.make_persistent_id("creation_destination");
                let original = self.creation.destination.clone();
                let rejected = crate::text_input::reject_events_with_limits(
                    ui,
                    input_id,
                    &original,
                    MAX_DESTINATION_BYTES,
                    MAX_DESTINATION_BYTES,
                );
                if rejected {
                    self.creation.issued = None;
                    self.creation.path_too_long = true;
                }
                let response = ui.add_enabled(
                    self.creation_available && !self.busy && self.creation.pending.is_none(),
                    TextEdit::singleline(&mut self.creation.destination)
                        .id(input_id)
                        .desired_width(400.0)
                        .char_limit(MAX_DESTINATION_BYTES),
                );
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_label(name_label);
                });
                if response.changed() {
                    self.creation.issued = None;
                    if self.creation.destination.len() > MAX_DESTINATION_BYTES {
                        self.creation.destination = original;
                        self.creation.path_too_long = true;
                    } else {
                        // Host validation still owns absolute path and native
                        // selection identity. A basename alone selects no folder.
                        self.creation.selected = self.creation.destination.contains(['/', '\\']);
                        self.creation.path_too_long = false;
                        self.creation.outcome = None;
                        self.error = None;
                    }
                }
                if self.creation.path_too_long {
                    ui.small(tr(ui, "creation-path-too-long"));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(tr(ui, "creation-location"));
                    if let Some((folder, _)) = self.creation.destination.rsplit_once(['/', '\\']) {
                        ui.label(folder);
                    }
                    if ui
                        .add_enabled(!self.busy, egui::Button::new(tr(ui, "creation-browse")))
                        .clicked()
                    {
                        actions.push(Action::ChooseCreationLocation);
                    }
                });
                if self.creation.selected
                    && !self
                        .creation
                        .destination
                        .to_ascii_lowercase()
                        .ends_with(".accdb")
                {
                    ui.small(tr(ui, "creation-invalid-name"));
                }
                if !self.creation.selected {
                    ui.small(tr(ui, "creation-choose-location"));
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.creation_ready(),
                            egui::Button::new(tr(ui, "creation-create")),
                        )
                        .clicked()
                        && let Some(request) = self.request_database_creation()
                    {
                        actions.push(Action::CreateDatabase { request });
                    }
                    if ui.button(tr(ui, "creation-cancel")).clicked() {
                        if self.creation.pending.is_some() {
                            actions.push(Action::Cancel);
                        } else {
                            self.creation.open = false;
                            self.backstage = Backstage::Home;
                        }
                    }
                });
                if let Some(key) = self.creation_status_key() {
                    ui.add_space(12.0);
                    ui.label(tr(ui, key));
                }
                if let Some(error) = &self.error {
                    ui.add_space(12.0);
                    diagnostics::show(ui, ui.make_persistent_id("creation-host-report"), error);
                }
            });
    }

    fn creation_status_key(&self) -> Option<&'static str> {
        if self.creation.pending.is_some() {
            return Some(if self.creation.cancelling {
                "creation-cancelling"
            } else {
                "creation-working"
            });
        }
        self.creation.outcome.map(|outcome| match outcome {
            CreationOutcome::Created => "creation-created",
            CreationOutcome::NotPublished => "creation-not-published",
            CreationOutcome::VisibilityUncertain => "creation-uncertain",
            CreationOutcome::PublishedUnconfirmed => "creation-unconfirmed",
            CreationOutcome::PublishedCleanupFailure => "creation-cleanup",
            CreationOutcome::Unsupported => "creation-unsupported",
        })
    }
}

#[cfg(test)]
mod tests;
