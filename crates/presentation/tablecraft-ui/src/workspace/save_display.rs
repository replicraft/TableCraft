//! Common reviewed offline-save status; these messages do not infer rollback.
use super::{EditSaveOutcome, State, diagnostics};
use crate::localization::tr;
use egui::Ui;

/// Host-reported terminal cleanup fact; this grants no file or recovery access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveCleanupNotice {
    RetainedPrivateJournal,
}

impl State {
    /// Keep acknowledged cleanup visible after a successful save dialog closes.
    /// A different document revision cannot inherit this report.
    pub fn set_save_cleanup(&mut self, revision: u64, notice: Option<SaveCleanupNotice>) {
        if self
            .database
            .as_ref()
            .is_some_and(|database| database.document_revision == revision)
        {
            self.save_cleanup_notice = notice.map(|notice| (revision, notice));
        }
    }

    pub(super) fn save_cleanup_status(&self) -> Option<&'static str> {
        let (revision, notice) = self.save_cleanup_notice?;
        if self.database.as_ref()?.document_revision != revision {
            return None;
        }
        Some(match notice {
            SaveCleanupNotice::RetainedPrivateJournal => "editing-save-retained-private-journal",
        })
    }
}

pub(super) fn pending(ui: &mut Ui, cancelled: bool, resolving: bool) {
    ui.spinner();
    ui.label(tr(
        ui,
        if resolving {
            "editing-resolving"
        } else {
            "editing-applying"
        },
    ));
    if cancelled {
        ui.label(tr(ui, "editing-cancellation-requested"));
    }
}

pub(super) fn outcome(ui: &mut Ui, outcome: &EditSaveOutcome) {
    let (heading, report) = match outcome {
        EditSaveOutcome::Retained {
            message,
            can_recover,
            can_release,
        } => {
            diagnostics::show(ui, ui.id().with("save_report"), message);
            ui.label(tr(ui, "editing-recovery-owned"));
            if !*can_recover && !*can_release {
                ui.label(tr(ui, "editing-diagnosis-required"));
            }
            return;
        }
        EditSaveOutcome::Saved(report) => ("editing-outcome-saved", report),
        EditSaveOutcome::Cancelled(report) => ("editing-outcome-cancelled", report),
        EditSaveOutcome::Failed(report) => ("editing-outcome-failed", report),
    };
    ui.heading(tr(ui, heading));
    diagnostics::show(ui, ui.id().with("save_report"), report);
}

#[cfg(test)]
mod tests;
