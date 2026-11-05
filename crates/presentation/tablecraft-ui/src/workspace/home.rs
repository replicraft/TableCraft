//! Working commands share the dense ribbon with explicitly disabled previews.
mod view;

use super::{Action, DocumentView, State, ribbon};
use ribbon::{CommandSize, Icon, command, group};

impl State {
    pub(super) fn home(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        ribbon::strip(ui, |ui| {
            group(ui, &t.text("ribbon-views"), |ui| {
                self.view_menu(ui, actions);
            });
            if self.ribbon_tab == ribbon::Tab::TableDesign && self.mock_preview {
                ribbon::design(ui, &t);
                return;
            }
            group(ui, &t.text("ribbon-clipboard"), |ui| {
                ui.horizontal(|ui| {
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &t,
                            Icon::Paste,
                            "ribbon-paste",
                            CommandSize::Large,
                        );
                    }
                    ui.vertical(|ui| {
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::Cut,
                                "ribbon-cut",
                                CommandSize::Small,
                            );
                        }
                        if command(
                            ui,
                            Icon::Copy,
                            &t.text("ribbon-copy"),
                            self.can_copy_selection(),
                            CommandSize::Small,
                        )
                        .on_hover_text(t.text(if self.record_selected() {
                            "ribbon-copy-record-help"
                        } else {
                            "ribbon-copy-cell-help"
                        }))
                        .clicked()
                        {
                            self.copy_selected(ui.ctx());
                        }
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::PaintBrush,
                                "ribbon-format-painter",
                                CommandSize::Small,
                            );
                        }
                    });
                });
            });
            if self.mock_preview {
                ribbon::home(ui, &t);
            }
        });
    }
}

#[cfg(test)]
mod tests;
