//! Existing appearance/language preferences in the shared Options dialog.
use crate::{Locale, Localizer, State, Theme};
use egui::Ui;

impl State {
    pub(in crate::workspace) fn appearance_options(&mut self, ui: &mut Ui) {
        // NO_CLONE: Share the immutable localization bundle across mutable widget work;
        // Localizer clones its Arc owner, not the catalogs.
        let localizer = self.localizer.clone();
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
}
