//! Source-bound SQL presentation and typed parameters never interpret query text.
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use crate::workspace::{Action, State, diagnostics};

impl State {
    pub(in crate::workspace) fn saved_query_editor(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) {
        let Some(target) = &self.saved_query.target else {
            ui.weak(tr(ui, "saved-query-choose"));
            return;
        };
        ui.heading(&target.name);
        ui.small(tr(ui, "saved-query-help"));
        if self.saved_query.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(ui, "saved-query-working"));
            });
        }
        let idle = !self.busy && self.saved_query.pending.is_none();
        let object_id = target.object_id;
        let document_revision = target.document_revision;
        self.saved_query_sql_editor(ui);
        if let Some(error) = &self.saved_query.error {
            let id = ui.make_persistent_id(("saved_query_error", document_revision, object_id));
            diagnostics::show(ui, id, error);
        }
        if !super::inputs_within_limits(&self.saved_query.inputs) {
            ui.small(tr(ui, "saved-query-input-limit"));
        }
        ui.horizontal_wrapped(|ui| {
            let run = ui.add_enabled(
                self.saved_query_command_ready(),
                egui::Button::new(tr(ui, "saved-query-run")),
            );
            focus::register(ui, Pane::Document, &run, false);
            if run.clicked()
                && let Some(action) = self.request_saved_query_run()
            {
                actions.push(action);
            }
            let inspect = ui.add_enabled(idle, egui::Button::new(tr(ui, "saved-query-inspect")));
            focus::register(ui, Pane::Document, &inspect, false);
            if inspect.clicked() {
                actions.push(Action::InspectSavedQuery {
                    // NO_CLONE: Return or dispatch an owned identity snapshot from retained
                    // shared State; the caller may queue work or mutate State while
                    // the source owner must remain intact.
                    target: self
                        .saved_query
                        .target
                        .as_ref()
                        .expect("selected saved query")
                        .clone(),
                });
            }
            self.query_run_limits_button(ui);
        });
        if self.saved_query.data.is_some()
            && (self.saved_query.pending.is_some()
                || self.saved_query.error.is_some()
                || self.saved_query_sql_dirty())
        {
            ui.small(tr(ui, "saved-query-previous-results"));
        }
        ui.separator();
    }
}
