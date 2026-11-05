//! Working File/Open and About surfaces using the observed Backstage rail layout.
mod rail;

use super::{Action, State, diagnostics, ribbon::Icon};
use crate::localization::tr;
use crate::{Locale, Localizer, Theme, theme::palette};
use egui::{Frame, Margin, ScrollArea, TextEdit, Ui};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Backstage {
    Closed,
    #[default]
    Open,
    About,
    Options,
}

impl State {
    pub(super) fn backstage(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        let colors = palette(ui.ctx());
        egui::Panel::left("file_rail")
            .exact_size(160.0)
            .frame(
                Frame::new()
                    .fill(colors.title_fill)
                    .inner_margin(Margin::symmetric(10, 10)),
            )
            .show(ui, |ui| {
                if rail::entry(ui, Icon::ArrowLeft, &tr(ui, "backstage-back"), None).clicked() {
                    self.dismiss_backstage();
                }
                ui.add_space(18.0);
                self.file_section(ui, Backstage::Open, Icon::FolderOpen, "backstage-open");

                let footer_height = 2.0 * rail::ROW_HEIGHT + ui.spacing().item_spacing.y;
                ui.add_space((ui.available_height() - footer_height).max(0.0));
                self.file_section(ui, Backstage::About, Icon::Info, "backstage-about");
                self.file_section(ui, Backstage::Options, Icon::Settings, "backstage-options");
            });
        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).fill(colors.surface))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt(("backstage_content", self.backstage))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.vertical(|ui| match self.backstage {
                                Backstage::Open => self.open_pane(ui, actions, path_id),
                                Backstage::About => self.about_pane(ui),
                                Backstage::Options => self.options_pane(ui),
                                Backstage::Closed => {}
                            });
                        });
                    });
            });
    }

    fn file_section(&mut self, ui: &mut Ui, view: Backstage, icon: Icon, key: &str) {
        ui.push_id(key, |ui| {
            if rail::entry(ui, icon, &tr(ui, key), Some(self.backstage == view)).clicked() {
                self.backstage = view;
            }
        });
    }

    fn open_pane(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        ui.heading(tr(ui, "backstage-open"));
        ui.label(tr(ui, "backstage-open-title"));
        ui.add_space(14.0);
        if let Some(error) = &self.error {
            diagnostics::show(ui, ui.id().with("open_error"), error);
            ui.add_space(8.0);
        }
        let path_label = ui.label(tr(ui, "backstage-path"));
        let path = ui
            .add_sized(
                [ui.available_width().min(620.0), 25.0],
                TextEdit::singleline(&mut self.path)
                    .id(path_id)
                    .hint_text(tr(ui, "backstage-path-hint")),
            )
            .labelled_by(path_label.id);
        crate::theme::input_border(ui, &path);
        if self.file_picker_available
            && ui
                .add_enabled(
                    self.file_picker_ready(),
                    egui::Button::new(tr(ui, "backstage-browse")),
                )
                .clicked()
        {
            actions.push(Action::ChooseFile);
        }
        ui.add_space(10.0);
        let password_label = ui.label(tr(ui, "backstage-password"));
        let password = ui
            .add_sized(
                [230.0, 25.0],
                TextEdit::singleline(&mut self.password)
                    .password(true)
                    .hint_text(tr(ui, "backstage-password-hint")),
            )
            .labelled_by(password_label.id);
        crate::theme::input_border(ui, &password);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.path.trim().is_empty(),
                    egui::Button::new(tr(ui, "backstage-open-database")),
                )
                .clicked()
            {
                actions.push(Action::Open);
            }
        });
        ui.add_space(12.0);
        ui.label(tr(ui, "backstage-stable-copy-help"));
        ui.small(tr(ui, "backstage-read-only-help"));
    }

    fn options_pane(&mut self, ui: &mut Ui) {
        let localizer = self.localizer.clone();
        ui.heading(localizer.text("settings-title"));
        ui.add_space(18.0);
        ui.heading(localizer.text("settings-appearance"));
        ui.label(localizer.text("settings-theme"));
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.theme,
                Theme::System,
                localizer.text("settings-theme-system"),
            );
            ui.selectable_value(
                &mut self.theme,
                Theme::Light,
                localizer.text("settings-light"),
            );
            ui.selectable_value(
                &mut self.theme,
                Theme::Dark,
                localizer.text("settings-dark"),
            );
        });
        ui.add_space(18.0);
        ui.label(localizer.text("settings-language"));
        let mut locale = localizer.locale();
        egui::ComboBox::from_id_salt("ui_language")
            .selected_text(locale.native_name())
            .show_ui(ui, |ui| {
                for choice in Locale::ALL {
                    ui.selectable_value(&mut locale, choice, choice.native_name());
                }
            });
        if locale != localizer.locale() {
            match Localizer::new(locale) {
                Ok(next) => self.localizer = next,
                Err(_) => {
                    ui.label(localizer.text("settings-translation-unavailable"));
                }
            }
            ui.ctx().request_repaint();
        }
        crate::formatting::show_options(ui, &mut self.formatting, &self.format_cache, &localizer);
        if self.mock_preview {
            ui.add_space(18.0);
            ui.label(localizer.text("settings-preview-help"));
        }
    }

    fn about_pane(&self, ui: &mut Ui) {
        ui.heading(tr(ui, "backstage-about-title"));
        ui.add_space(14.0);
        ui.label(tr(ui, "backstage-about-description"));
        ui.label(tr(ui, "backstage-font-credit"));
        ui.hyperlink_to(
            tr(ui, "backstage-font-source"),
            "https://github.com/microsoft/Selawik/releases/tag/1.01",
        );
        egui::CollapsingHeader::new(tr(ui, "backstage-font-license")).show(ui, |ui| {
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(include_str!(
                    "../../../../../assets/ui/fonts/OFL-Selawik.txt"
                ));
            });
        });
        ui.add_space(12.0);
        ui.label(tr(ui, "backstage-noto-credit"));
        ui.hyperlink_to(
            tr(ui, "backstage-noto-source"),
            "https://github.com/notofonts/noto-cjk",
        );
        egui::CollapsingHeader::new(tr(ui, "backstage-noto-license")).show(ui, |ui| {
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(include_str!(
                    "../../../../../assets/ui/fonts/OFL-NotoSansCJK.txt"
                ));
            });
        });
        ui.add_space(12.0);
        ui.label(tr(ui, "backstage-icons-credit"));
        ui.hyperlink_to(
            tr(ui, "backstage-icons-source"),
            "https://github.com/replicraft/TableCraft/tree/main/assets/ui/icons/tablecraft",
        );
        egui::CollapsingHeader::new(tr(ui, "backstage-icons-license")).show(ui, |ui| {
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(include_str!("../../../../../LICENSE"));
            });
        });
    }
}

#[cfg(test)]
mod tests;
