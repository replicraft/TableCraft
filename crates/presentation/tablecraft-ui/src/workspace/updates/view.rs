use super::{Request, UpdateFailure, UpdateManualReason, UpdateStatus};
use crate::{
    localization::{message, tr},
    workspace::{Action, State},
};
use egui::{Button, Frame, ProgressBar, Ui};

const SECTION_WIDTH: f32 = 620.0;

impl State {
    pub(in crate::workspace) fn update_section(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let generation = self.updates.generation;
        ui.push_id(("update-controls", generation), |ui| {
            Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
                ui.set_max_width(SECTION_WIDTH);
                ui.heading(tr(ui, "updates-title"));
                ui.label(message(
                    ui,
                    "updates-installed",
                    &[("version", env!("CARGO_PKG_VERSION"))],
                ));
                ui.add_space(8.0);
                status(ui, &self.updates.status);
                ui.add_space(8.0);

                let available = self.updates.available && !self.mock_preview;
                let request = match &self.updates.status {
                    UpdateStatus::Idle
                    | UpdateStatus::Current
                    | UpdateStatus::Failed(_)
                    | UpdateStatus::Manual { .. } => {
                        command(ui, "updates-check", available).then_some(Request::Check)
                    }
                    UpdateStatus::Available { version } => ui
                        .push_id(version.as_str(), |ui| {
                            command(ui, "updates-download", available)
                        })
                        .inner
                        .then_some(Request::Download),
                    UpdateStatus::Checking | UpdateStatus::Downloading { .. } => {
                        command(ui, "updates-cancel", available).then_some(Request::Cancel)
                    }
                    UpdateStatus::Ready { version } => {
                        let ready =
                            available && self.updates.can_install && self.can_restart_for_update();
                        let clicked = ui
                            .push_id(("updates-install", version.as_str()), |ui| {
                                ui.add_enabled(ready, Button::new(tr(ui, "updates-install")))
                                    .on_disabled_hover_text(tr(
                                        ui,
                                        if available {
                                            "updates-save-first"
                                        } else {
                                            "ribbon-unavailable"
                                        },
                                    ))
                                    .clicked()
                            })
                            .inner;
                        if available && !ready {
                            ui.small(tr(ui, "updates-save-first"));
                        }
                        clicked.then_some(Request::Install)
                    }
                    UpdateStatus::Installing | UpdateStatus::Cancelling => None,
                };
                if let Some(request) = request
                    && let Some(action) = self.request_update(request)
                {
                    actions.push(action);
                }
            });
        });
    }
}

fn command(ui: &mut Ui, key: &str, available: bool) -> bool {
    ui.push_id(key, |ui| {
        ui.add_enabled(available, Button::new(tr(ui, key)))
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"))
            .clicked()
    })
    .inner
}

fn status(ui: &mut Ui, status: &UpdateStatus) {
    match status {
        UpdateStatus::Idle => {
            ui.label(tr(ui, "updates-idle"));
        }
        UpdateStatus::Checking => pending(ui, "updates-checking"),
        UpdateStatus::Current => {
            ui.label(tr(ui, "updates-current"));
        }
        UpdateStatus::Available { version } => {
            ui.label(message(ui, "updates-available", &[("version", version)]));
        }
        UpdateStatus::Downloading { received, total } => {
            if *total == 0 {
                pending(ui, "updates-preparing-download");
            } else {
                let progress = (*received as f64 / *total as f64).min(1.0) as f32;
                ui.add(ProgressBar::new(progress).show_percentage());
                ui.label(message(
                    ui,
                    "updates-downloading",
                    &[
                        ("received", received.to_string()),
                        ("total", total.to_string()),
                    ],
                ));
            }
        }
        UpdateStatus::Ready { version } => {
            ui.label(message(ui, "updates-ready", &[("version", version)]));
        }
        UpdateStatus::Installing => pending(ui, "updates-installing"),
        UpdateStatus::Cancelling => pending(ui, "updates-cancelling"),
        UpdateStatus::Failed(failure) => {
            ui.colored_label(ui.visuals().error_fg_color, tr(ui, failure_key(*failure)));
        }
        UpdateStatus::Manual { version, reason } => {
            if let Some(version) = version {
                ui.label(message(ui, "updates-available", &[("version", version)]));
            }
            ui.label(tr(ui, manual_key(*reason)));
        }
    }
}

fn pending(ui: &mut Ui, key: &str) {
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(tr(ui, key));
    });
}

fn failure_key(failure: UpdateFailure) -> &'static str {
    match failure {
        UpdateFailure::Network => "updates-error-network",
        UpdateFailure::Signature => "updates-error-signature",
        UpdateFailure::Storage => "updates-error-storage",
        UpdateFailure::InvalidRelease => "updates-error-release",
        UpdateFailure::InvalidArchive => "updates-error-archive",
        UpdateFailure::Installation => "updates-error-installation",
        UpdateFailure::NotConfigured => "updates-error-not-configured",
        UpdateFailure::UnsupportedPlatform => "updates-error-platform",
    }
}

fn manual_key(reason: UpdateManualReason) -> &'static str {
    match reason {
        UpdateManualReason::DevelopmentBuild => "updates-manual-development",
        UpdateManualReason::ManagedInstallation => "updates-manual-managed",
        UpdateManualReason::UnrecognizedInstallation => "updates-manual-unrecognized",
        UpdateManualReason::UnsupportedPlatform => "updates-manual-platform",
    }
}
