//! Working File/Open and About surfaces using the observed Backstage rail layout.
mod home;
mod rail;

use super::{Action, State, diagnostics, ribbon::Icon};
use crate::localization::tr;
use crate::{Locale, Localizer, Theme, theme::palette};
use egui::{Frame, Margin, ScrollArea, TextEdit, Ui};

const FOOTER_ENTRIES: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum Backstage {
    Closed,
    #[default]
    Home,
    Open,
    New,
    About,
    Options,
}

impl State {
    pub(super) fn backstage(&mut self, ui: &mut Ui, actions: &mut Vec<Action>, path_id: egui::Id) {
        if self.backstage == Backstage::Home && self.database.is_none() {
            self.startup(ui, actions);
            return;
        }
        let colors = palette(ui.ctx());
        let rail_width = rail::width(ui, &self.localizer);
        egui::Panel::left("file_rail")
            .exact_size(rail_width)
            .frame(
                Frame::new()
                    .fill(colors.title_fill)
                    .inner_margin(Margin::symmetric(rail::OUTER_PADDING, 10)),
            )
            .show(ui, |ui| {
                if self.backstage_return_available() {
                    if rail::entry(ui, Icon::ArrowLeft, &tr(ui, "backstage-back"), None).clicked() {
                        self.return_from_backstage();
                    }
                    ui.add_space(18.0);
                }

                let footer_height =
                    FOOTER_ENTRIES as f32 * (rail::ROW_HEIGHT + ui.spacing().item_spacing.y);
                ScrollArea::vertical()
                    .id_salt("file_rail_sections")
                    .max_height((ui.available_height() - footer_height).max(rail::ROW_HEIGHT))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        self.file_section(ui, Backstage::Home, Icon::Home, "backstage-home");
                        self.file_section(ui, Backstage::New, Icon::NewDocument, "backstage-new");
                        self.file_section(ui, Backstage::Open, Icon::FolderOpen, "backstage-open");
                        ui.separator();
                        for key in [
                            "backstage-info",
                            "backstage-save",
                            "backstage-save-as",
                            "backstage-print",
                            "backstage-close",
                        ] {
                            self.unavailable_file_section(ui, key);
                        }
                    });
                ui.add_space((ui.available_height() - footer_height).max(0.0));
                self.unavailable_file_section(ui, "backstage-account");
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
                                Backstage::Home => self.start_pane(ui, actions),
                                Backstage::New => self.new_pane(ui, actions),
                                Backstage::Open => self.open_pane(ui, actions, path_id),
                                Backstage::About => self.about_pane(ui),
                                Backstage::Options => self.options_pane(ui),
                                Backstage::Closed => {}
                            });
                        });
                    });
            });
    }

    pub(super) fn backstage_return_available(&self) -> bool {
        self.database.is_some() || self.backstage != Backstage::Home
    }

    pub(super) fn return_from_backstage(&mut self) {
        if self.database.is_some() {
            self.dismiss_backstage();
        } else {
            self.backstage = Backstage::Home;
        }
    }

    fn unavailable_file_section(&self, ui: &mut Ui, key: &str) {
        ui.push_id(key, |ui| {
            ui.add_enabled_ui(false, |ui| {
                let icon = match key {
                    "backstage-info" => Icon::Info,
                    "backstage-save" => Icon::Save,
                    "backstage-save-as" => Icon::SaveAs,
                    "backstage-print" => Icon::Print,
                    "backstage-close" => Icon::CloseDocument,
                    "backstage-account" => Icon::Account,
                    _ => return,
                };
                rail::entry(ui, icon, &tr(ui, key), None)
                    .on_disabled_hover_text(tr(ui, "backstage-unavailable"));
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
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(140.0);
                ui.label(tr(ui, "backstage-computer"));
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
            });
            ui.vertical(|ui| {
                ui.heading(tr(ui, "backstage-recent"));
                ui.label(tr(ui, "backstage-recent-empty"));
            });
        });
        ui.add_space(12.0);
        ui.separator();
        ui.label(tr(ui, "backstage-open-title"));
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
        let language_label = ui.label(localizer.text("settings-language"));
        let mut locale = localizer.locale();
        egui::ComboBox::from_id_salt("ui_language")
            .selected_text(locale.native_name())
            .show_ui(ui, |ui| {
                for choice in Locale::ALL {
                    ui.selectable_value(&mut locale, choice, choice.native_name());
                }
            })
            .response
            .labelled_by(language_label.id);
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
        ui.label(tr(ui, "backstage-emoji-credit"));
        ui.hyperlink_to(
            tr(ui, "backstage-emoji-source"),
            "https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notoemoji",
        );
        egui::CollapsingHeader::new(tr(ui, "backstage-emoji-license")).show(ui, |ui| {
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.label(include_str!(
                    "../../../../../assets/ui/fonts/OFL-NotoEmoji.txt"
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
