//! Missing native publications are distinct from a loaded empty table.
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use crate::workspace::objects::ObjectKind;
use crate::workspace::{Action, DocumentView, State};
use egui::{Label, RichText, Ui};

impl State {
    pub(super) fn unloaded_object_status(&self, ui: &mut Ui, actions: &mut Vec<Action>) -> bool {
        let Some(open) = self.active_object_load() else {
            return false;
        };
        let ObjectKind::Table { name } = &open.tab.target.kind else {
            return false;
        };
        if !open.needs_load || !matches!(self.view, DocumentView::Table | DocumentView::Design) {
            return false;
        }

        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.add(Label::new(RichText::new(name).strong()).truncate())
                .on_hover_text(name);
            ui.add_space(8.0);
            let status = if self.busy {
                "objects-loading"
            } else if self.error.is_some() || self.filter_error_message().is_some() {
                "objects-load-failed"
            } else {
                "objects-not-loaded"
            };
            ui.label(tr(ui, status));
            if self.busy {
                ui.spinner();
            } else {
                ui.add_space(8.0);
                let retry = ui.add_enabled(
                    self.can_retry_object(&open.tab),
                    egui::Button::new(tr(ui, "objects-retry")),
                );
                focus::register(ui, Pane::Document, &retry, true);
                if retry.clicked() {
                    actions.push(Action::RetryObject { tab: open.tab });
                }
            }
        });
        true
    }
}

#[cfg(test)]
mod tests;
