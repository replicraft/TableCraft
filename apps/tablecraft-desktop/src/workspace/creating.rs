//! Desktop composition of the retained shared creation operation and File/New UI.
use super::{Desktop, table_drafts::DeferredDraftAction};
use crate::file_dialog::SelectedFile;
use std::{path::PathBuf, sync::Arc};
use tablecraft_runtime::{
    Command, CreationOutcome as RuntimeOutcome, CreationPublication, CreationReport,
    CreationRequest as RuntimeRequest, CreationStartFailure, CreationWorker,
};
use tablecraft_ui::{CreationOutcome, CreationRequest};
use zeroize::Zeroize;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;

pub(super) struct Creating {
    worker: CreationWorker,
    selected: Option<SelectedFile>,
    active: Option<Active>,
    terminal: Option<Arc<CreationReport>>,
}

struct Active {
    operation: u64,
    request: u64,
}

impl Creating {
    pub(super) fn new(context: egui::Context) -> Self {
        Self {
            worker: CreationWorker::new(move || context.request_repaint()),
            selected: None,
            active: None,
            terminal: None,
        }
    }

    pub(super) fn is_busy(&self) -> bool {
        self.active.is_some() || self.worker.is_busy()
    }
}

impl Desktop {
    pub(super) fn creation_selected(&mut self, selection: Option<PathBuf>) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.state.busy
            || self.state.table_draft_field_delete_open()
        {
            return;
        }
        if let Some(path) = selection {
            let selected = SelectedFile::new(path);
            if self
                .state
                .set_creation_destination(selected.display().to_owned())
            {
                self.creating.selected = Some(selected);
            } else {
                self.state.error = Some(self.state.localizer.text("creation-path-too-long"));
            }
        }
    }

    pub(super) fn invalidate_creation_selection(&mut self) {
        if self.creating.selected.as_ref().is_some_and(|selected| {
            selected
                .path_for(self.state.creation_destination())
                .is_none()
        }) {
            self.creating.selected = None;
        }
    }

    pub(super) fn create_database(&mut self, request: CreationRequest) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.state.insertion_dialog_open()
            || self.state.insert_continuation_pending()
            || self.state.table_draft_field_delete_open()
        {
            return;
        }
        let Some(DeferredDraftAction::Create(request)) =
            self.guard_table_draft_action(DeferredDraftAction::Create(request))
        else {
            return;
        };
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || !self.state.begin_database_creation(&request)
        {
            return;
        }
        let destination = self
            .creating
            .selected
            .as_ref()
            .and_then(|selected| selected.path_for(&request.destination))
            .map_or_else(|| PathBuf::from(&request.destination), PathBuf::from);
        let absolute = destination.is_absolute();
        let valid_extension = destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("accdb"));
        if !absolute || !valid_extension {
            self.state
                .finish_database_creation(request.id, CreationOutcome::NotPublished);
            let key = if absolute {
                "creation-invalid-name"
            } else {
                "creation-location-needed"
            };
            self.state.error = Some(self.state.localizer.text(key));
            return;
        }

        match self
            .creating
            .worker
            .start(RuntimeRequest::blank(destination))
        {
            Ok(operation) => {
                self.worker.cancel();
                self.creating.active = Some(Active {
                    operation,
                    request: request.id,
                });
                self.creating.terminal = None;
                self.state.error = None;
            }
            Err(error) => {
                let outcome = if error.kind == CreationStartFailure::UnsupportedPlatform {
                    CreationOutcome::Unsupported
                } else {
                    CreationOutcome::NotPublished
                };
                self.state.finish_database_creation(request.id, outcome);
            }
        }
    }

    pub(super) fn poll_creation(&mut self) {
        let Some(active) = &self.creating.active else {
            return;
        };
        let Some(report) = self.creating.worker.poll() else {
            return;
        };
        if report.id != active.operation {
            self.state.error = Some(self.state.localizer.text("creation-uncertain"));
            return;
        }
        let outcome = match report.outcome() {
            RuntimeOutcome::Created(_) => CreationOutcome::Created,
            RuntimeOutcome::Failed(failure) => match failure.publication {
                CreationPublication::NotPublished { .. } => CreationOutcome::NotPublished,
                CreationPublication::VisibilityUncertain => CreationOutcome::VisibilityUncertain,
                CreationPublication::PublishedUnconfirmed => CreationOutcome::PublishedUnconfirmed,
                CreationPublication::PublishedCleanupFailure { .. } => {
                    CreationOutcome::PublishedCleanupFailure
                }
            },
        };
        if !self.state.finish_database_creation(active.request, outcome) {
            self.state.error = Some(self.state.localizer.text("creation-uncertain"));
            return;
        }

        // Keep the complete receipt/failure after acknowledging worker ownership.
        // Opening is a separate guarded snapshot operation and can fail without
        // erasing the known fact that publication succeeded.
        // Creation terminal report remains retained after worker acknowledgement while an
        // independently guarded OpenCreated command owns the same immutable report.
        self.creating.terminal = Some(Arc::clone(&report));
        if !self.creating.worker.acknowledge(report.id) {
            self.state.error = Some(self.state.localizer.text("creation-uncertain"));
            return;
        }
        self.creating.active = None;
        if outcome == CreationOutcome::Created {
            self.select_file(report.destination().to_path_buf());
            self.state.password.zeroize();
            self.state.password.clear();
            self.open_request(Command::OpenCreated { report });
        }
    }

    pub(super) fn cancel_creation(&mut self) {
        if let Some(active) = &self.creating.active
            && self.creating.worker.cancel(active.operation)
        {
            self.state.mark_creation_cancelling(active.request);
        }
    }

    pub(super) fn guard_creation_close(&mut self, context: &egui::Context) {
        if self.creating.is_busy() && context.input(|input| input.viewport().close_requested()) {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel_creation();
        }
    }
}
