//! Common reviewed offline-save status; these messages do not infer rollback.
use super::{EditSaveOutcome, diagnostics};
use crate::localization::tr;
use egui::Ui;

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
