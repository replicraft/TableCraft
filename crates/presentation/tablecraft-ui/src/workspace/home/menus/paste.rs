//! The Paste menu stages an exact UI target before requesting clipboard text.
use super::{
    Action, CommandSize, Icon, ObjectTab, Popup, State, Ui, command, command_menu, show_menu,
};
use crate::EditTarget;

const MAX_FOCUS_FRAMES: u64 = 8;

#[derive(Clone, PartialEq, Eq)]
struct PasteMenuWitness {
    target: EditTarget,
    publication: u64,
    object: Option<ObjectTab>,
}

pub(in crate::workspace) struct PendingPaste {
    witness: PasteMenuWitness,
    started: u64,
    previous_frame: u64,
    // Comparison only: focus is requested from the next real selected cell response.
    focus: Option<egui::Id>,
}

impl State {
    pub(in crate::workspace) fn paste_menu(&mut self, ui: &mut Ui) {
        // NO_CLONE: Share the immutable localization bundle across mutable widget work;
        // Localizer clones its Arc owner, not the catalogs.
        let t = self.localizer.clone();
        let current = self.current_paste_target().map(|target| PasteMenuWitness {
            target,
            publication: self.focus_publication,
            object: self.active_object(),
        });
        let response = command_menu(
            ui,
            Icon::Paste,
            &t.text("ribbon-paste"),
            current.is_some(),
            CommandSize::Large,
        )
        .on_hover_text(t.text("editing-paste-help"));
        let popup = Popup::default_response_id(&response);
        let witness_id = response.id.with("paste_text_target");
        let captured = ui.data(|data| data.get_temp::<PasteMenuWitness>(witness_id));
        if Popup::is_id_open(ui.ctx(), popup) && captured != current {
            Popup::close_id(ui.ctx(), popup);
        }
        if !Popup::is_id_open(ui.ctx(), popup) {
            ui.data_mut(|data| data.remove::<PasteMenuWitness>(witness_id));
        }
        show_menu(ui, &response, |ui| {
            let Some(current) = current else {
                return;
            };
            let captured = ui.data_mut(|data| {
                // NO_CLONE: the retained command or exact-owner witness needs its own captured
                // identity after this render/dispatch; keep the original catalog,
                // input or owner available for the later checks in this operation.
                let captured = data
                    .get_temp::<PasteMenuWitness>(witness_id)
                    .unwrap_or_else(|| current.clone());
                // NO_CLONE: the retained command or exact-owner witness needs its own captured
                // identity after this render/dispatch; keep the original catalog,
                // input or owner available for the later checks in this operation.
                data.insert_temp(witness_id, captured.clone());
                captured
            });
            let enabled = captured == current && self.can_paste_cell(&captured.target);
            if command(
                ui,
                Icon::Paste,
                &t.text("editing-paste-text"),
                enabled,
                CommandSize::Small,
            )
            .on_hover_text(t.text("editing-paste-help"))
            .clicked()
            {
                let frame = ui.ctx().cumulative_frame_nr();
                self.ribbon_paste = Some(PendingPaste {
                    witness: captured,
                    started: frame,
                    previous_frame: frame,
                    focus: ui.ctx().memory(|memory| memory.focused()),
                });
                self.table_selection.focus_selected(ui.ctx());
                ui.close();
            }
            for key in ["ribbon-paste-special", "ribbon-paste-append"] {
                command(ui, Icon::Paste, &t.text(key), false, CommandSize::Small)
                    .on_disabled_hover_text(t.text("ribbon-unavailable"));
            }
        });
    }

    /// Issue the clipboard request only after the exact selected native cell has
    /// published its actual focusable response. Failed reveal never retargets it.
    pub(in crate::workspace) fn finish_ribbon_paste(&mut self, ui: &Ui, actions: &[Action]) {
        let Some(mut pending) = self.ribbon_paste.take() else {
            return;
        };
        let frame = ui.ctx().cumulative_frame_nr();
        let current = self.current_paste_target().map(|target| PasteMenuWitness {
            target,
            publication: self.focus_publication,
            object: self.active_object(),
        });
        let consecutive =
            frame == pending.previous_frame || frame == pending.previous_frame.saturating_add(1);
        let source_current = current.as_ref() == Some(&pending.witness)
            && consecutive
            && frame.saturating_sub(pending.started) <= MAX_FOCUS_FRAMES;
        if !source_current {
            self.error = Some(self.localizer.text("editing-paste-stale"));
            return;
        }
        if actions
            .iter()
            .any(|action| matches!(action, Action::PasteCell { .. }))
        {
            return;
        }
        if self.table_selection.focused_cell_ready(ui.ctx()) {
            self.request_paste(ui.ctx());
            if self.ribbon_temporarily_open() {
                // The delayed reply now belongs to the real document cell, so
                // closing the temporary presentation cannot remove its focus owner.
                self.set_ribbon_collapsed(true);
            }
        } else if ui.ctx().memory(|memory| memory.focused()) == pending.focus {
            pending.previous_frame = frame;
            self.ribbon_paste = Some(pending);
            ui.ctx().request_repaint();
        } else {
            self.error = Some(self.localizer.text("editing-paste-stale"));
        }
    }
}
