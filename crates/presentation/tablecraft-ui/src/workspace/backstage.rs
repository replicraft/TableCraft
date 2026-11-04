//! Working File/Open and About surfaces using the observed Backstage rail layout.
use super::{Action, State};
use crate::theme::{ACCENT, TITLE_FILL};
use egui::{Color32, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Backstage {
    Closed,
    #[default]
    Open,
    About,
}

impl State {
    pub(super) fn backstage(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        egui::Panel::left("file_rail")
            .exact_size(160.0)
            .frame(
                Frame::new()
                    .fill(TITLE_FILL)
                    .inner_margin(Margin::symmetric(12, 10)),
            )
            .show(ui, |ui| {
                if ui.button("‹ Back").clicked() {
                    self.dismiss_backstage();
                }
                ui.add_space(18.0);
                for (view, title) in [(Backstage::Open, "Open"), (Backstage::About, "About")] {
                    let active = self.backstage == view;
                    let text = if active {
                        RichText::new(title).color(ACCENT)
                    } else {
                        RichText::new(title)
                    };
                    let response = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(text).selected(active).fill(TITLE_FILL),
                    );
                    if active {
                        ui.painter().line_segment(
                            [response.rect.left_top(), response.rect.left_bottom()],
                            egui::Stroke::new(2.0, ACCENT),
                        );
                    }
                    if response.clicked() {
                        self.backstage = view;
                    }
                }
            });
        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).fill(Color32::WHITE))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("backstage_content")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.vertical(|ui| match self.backstage {
                                Backstage::Open => self.open_pane(ui, actions, path_id),
                                Backstage::About => self.about_pane(ui),
                                Backstage::Closed => {}
                            });
                        });
                    });
            });
    }

    fn open_pane(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        ui.heading("Open");
        ui.label("Open a local Access database");
        ui.add_space(14.0);
        if let Some(error) = &self.error {
            ScrollArea::vertical()
                .id_salt("open_error")
                .max_height(96.0)
                .show(ui, |ui| {
                    ui.colored_label(ACCENT, error);
                });
            ui.add_space(8.0);
        }
        let path_label = ui.label("Database path");
        ui.add_sized(
            [ui.available_width().min(620.0), 25.0],
            TextEdit::singleline(&mut self.path)
                .id(path_id)
                .hint_text("Path to .accdb or .mdb"),
        )
        .labelled_by(path_label.id);
        if self.file_picker_available
            && ui
                .add_enabled(self.file_picker_ready(), egui::Button::new("Browse…"))
                .clicked()
        {
            actions.push(Action::ChooseFile);
        }
        ui.add_space(10.0);
        let password_label = ui.label("Password");
        ui.add_sized(
            [230.0, 25.0],
            TextEdit::singleline(&mut self.password)
                .password(true)
                .hint_text("Optional password"),
        )
        .labelled_by(password_label.id);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.path.trim().is_empty(),
                    egui::Button::new("Open database"),
                )
                .clicked()
            {
                actions.push(Action::Open);
            }
        });
        ui.add_space(12.0);
        ui.label("Open a stable copy or close other database writers before opening.");
        ui.small(
            "Opening does not change the file. Offline edits require explicit review and apply.",
        );
    }

    fn about_pane(&self, ui: &mut Ui) {
        ui.heading("About TableCraft");
        ui.add_space(14.0);
        ui.label("A cross-platform Access workspace. Desktop preview.");
        ui.label("Selawik regular · © 2015 Microsoft Corporation");
        ui.hyperlink_to(
            "Font source: Microsoft Selawik 1.01",
            "https://github.com/microsoft/Selawik/releases/tag/1.01",
        );
        egui::CollapsingHeader::new("Selawik license").show(ui, |ui| {
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(include_str!(
                    "../../../../../assets/ui/fonts/OFL-Selawik.txt"
                ));
            });
        });
    }
}

#[cfg(test)]
mod tests;
